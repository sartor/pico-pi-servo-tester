#![no_std]
#![no_main]

use core::sync::atomic::{AtomicI16, AtomicU8, Ordering};
use defmt::*;
use embassy_rp::{bind_interrupts, gpio:: {Pin, AnyPin, Level, Output, Pull}, adc, pwm, peripherals};
use embassy_time::Timer;
use fixed::traits::ToFixed;
use {defmt_rtt as _, panic_probe as _};

mod debouncer;

static MODES: [[u16; 2]; 3] = [
    [1000, 2000],
    [750, 2250],
    [500, 2500],
];

#[derive(PartialEq)]
#[derive(Format)]
enum Button {
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

static CURRENT_MODE_INDEX: AtomicU8 = AtomicU8::new(0);
static LAST_ADC: AtomicI16 = AtomicI16::new(ADC_BUTTONS[5].0);

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
});

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    spawner.spawn(run_led(p.PIN_25.degrade())).unwrap();
    spawner.spawn(run_adc(p.PIN_26, p.ADC)).unwrap();
    spawner.spawn(run_pwm(p.PIN_27, p.PWM_CH5)).unwrap();
}

#[embassy_executor::task]
async fn run_led(led_pin: AnyPin) {
    let mut led = Output::new(led_pin, Level::Low);

    loop {
        //info!("led on!");
        led.set_high();
        Timer::after_secs(1).await;

        //info!("led off!");
        led.set_low();
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn run_adc(adc_pin: peripherals::PIN_26, p_adc: peripherals::ADC) {

    let mut adc = adc::Adc::new(p_adc, Irqs, adc::Config::default());

    let mut p26 = adc::Channel::new_pin(adc_pin, Pull::Up);

    loop {
        let level = adc.read(&mut p26).await.unwrap() as i16;
        let last_button = adc_to_button(LAST_ADC.load(Ordering::Relaxed));
        let cur_button = adc_to_button(level);
        if cur_button != &Button::None && last_button == &Button::None {
            handle_button(cur_button);
        }
        LAST_ADC.store(level, Ordering::Relaxed);

        Timer::after_millis(10).await;
    }
}

#[embassy_executor::task]
async fn run_pwm(pwm_pin: peripherals::PIN_27, pwm_channel: peripherals::PWM_CH5) {

    let mut c: pwm::Config = Default::default();
    c.divider = 125.to_fixed();
    c.top = 20000;
    c.compare_b = 1500;
    let mut pwm = pwm::Pwm::new_output_b(pwm_channel, pwm_pin, c.clone());

    loop {
        //info!("current LED duty cycle: {}", c.compare_b);
        Timer::after_millis(300).await;

        if c.compare_b < 2500 {
            c.compare_b += 100;
        } else {
            c.compare_b = 500;
        }
        pwm.set_config(&c);
    }
}

fn adc_to_button(adc_value: i16) -> &'static Button {
    ADC_BUTTONS
        .iter()
        .min_by_key(|(comp_value, _)| (comp_value - adc_value).abs())
        .map(|(_, button)| button)
        .unwrap()
}

fn handle_button(button: &Button) {
    info!("Button pressed: {}", button);
}