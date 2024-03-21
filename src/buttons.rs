use core::sync::atomic::{AtomicI16, Ordering};
use defmt::*;
use embassy_rp::adc;
use embassy_time::Timer;

#[derive(PartialEq)]
#[derive(Format)]
pub enum Button {
    None,
    Up,
    Down,
    Left,
    Right,
    Center,
}

const ADC_BUTTONS: [(i16, Button); 6] = [
    (90, Button::Down),
    (585, Button::Right),
    (1155, Button::Up),
    (1835, Button::Left),
    (2455, Button::Center),
    (3990, Button::None),
];
static LAST_ADC: AtomicI16 = AtomicI16::new(ADC_BUTTONS[5].0);

pub async fn wait_for_button(adc: &mut adc::Adc<'_, adc::Async>, pin: &mut adc::Channel<'_>) -> &'static Button {
    loop {
        let level = adc.read(pin).await.unwrap() as i16;
        let last_button = adc_to_button(LAST_ADC.load(Ordering::Relaxed));
        LAST_ADC.store(level, Ordering::Relaxed);
        let cur_button = adc_to_button(level);
        if cur_button != &Button::None && last_button == &Button::None {
            return cur_button;
        }
        Timer::after_millis(10).await;
    }
}

fn adc_to_button(adc_value: i16) -> &'static Button {
    ADC_BUTTONS
        .iter()
        .min_by_key(|(comp_value, _)| (comp_value - adc_value).abs())
        .map(|(_, button)| button)
        .unwrap()
}