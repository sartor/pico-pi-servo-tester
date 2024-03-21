#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use defmt::*;
use embassy_rp::{bind_interrupts, gpio::Pull, adc, pwm, peripherals, pio};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use fixed::traits::ToFixed;
use {defmt_rtt as _, panic_probe as _};
use crate::buttons::Button;

mod buttons;
mod pio_ws2812;

static MODES: [[u16; 2]; 3] = [
    [1000, 2000],
    [750, 2250],
    [500, 2500],
];

static CURRENT_MODE_INDEX: AtomicUsize = AtomicUsize::new(0);
static CURRENT_SIDE_INDEX: AtomicUsize = AtomicUsize::new(0);
static RUNNING: AtomicBool = AtomicBool::new(true);
bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

type PwmType = Mutex<ThreadModeRawMutex, Option<pwm::Pwm<'static, peripherals::PWM_CH5>>>;
type PwmConfigType = Mutex<ThreadModeRawMutex, Option<pwm::Config>>;
static PWM: PwmType = Mutex::new(None);
static PWM_CONFIG: PwmConfigType = Mutex::new(None);

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut c: pwm::Config = Default::default();
    c.divider = 125.to_fixed();
    c.top = 20000;
    c.compare_b = 1000;
    let pwm = pwm::Pwm::new_output_b(p.PWM_CH5, p.PIN_27, c.clone());
    {
        *(PWM.lock().await) = Some(pwm);
        *(PWM_CONFIG.lock().await) = Some(c);
    }

    let pio::Pio { mut common, sm0, .. } = pio::Pio::new(p.PIO0, Irqs);

    let ws2812 = pio_ws2812::Ws2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_16);

    spawner.spawn(run_main(ws2812)).unwrap();
    spawner.spawn(run_adc(p.PIN_26, p.ADC)).unwrap();
    spawner.spawn(run_pwm()).unwrap();
}

#[embassy_executor::task]
async fn run_main(mut ws2812: pio_ws2812::Ws2812<'static, peripherals::PIO0, 0, 1>) {

    let mut color;
    let mut last_pwm = 0;

    loop {
        update_pwm().await;

        let mode = CURRENT_MODE_INDEX.load(Ordering::Relaxed);
        let side = CURRENT_SIDE_INDEX.load(Ordering::Relaxed);
        let pwm_value = MODES[mode][side];

        if last_pwm != pwm_value {
            debug!("PWM: {}", pwm_value);
        }

        if !RUNNING.load(Ordering::Relaxed) {
            color = pio_ws2812::color_gray()
        } else {
            color = pio_ws2812::color_wheel(((pwm_value - 500) / 10) as u8);
        }

        ws2812.write(&[color]).await;
        Timer::after_millis(50).await;
        last_pwm = pwm_value;
    }
}

#[embassy_executor::task]
async fn run_adc(adc_pin: peripherals::PIN_26, p_adc: peripherals::ADC) {
    let mut adc = adc::Adc::new(p_adc, Irqs, adc::Config::default());
    let mut pin = adc::Channel::new_pin(adc_pin, Pull::Up);

    loop {
        let button = buttons::wait_for_button(&mut adc, &mut pin).await;
        let cur_mode = CURRENT_MODE_INDEX.load(Ordering::Relaxed);
        match button {
            Button::Up => {
                CURRENT_MODE_INDEX.store(if cur_mode == 0 { 0 } else { cur_mode - 1 }, Ordering::Relaxed)
            }
            Button::Down => {
                CURRENT_MODE_INDEX.store(if cur_mode == 2 { 2 } else { cur_mode + 1 }, Ordering::Relaxed)
            }
            Button::Left => {
                CURRENT_SIDE_INDEX.store(1, Ordering::Relaxed)
            }
            Button::Right => {
                CURRENT_SIDE_INDEX.store(0, Ordering::Relaxed)
            }
            Button::Center => {
                RUNNING.store(!RUNNING.load(Ordering::Relaxed), Ordering::Relaxed)
            }
            Button::None => {}
        }
        info!("Button pressed: {}", button);
    }
}

#[embassy_executor::task]
async fn run_pwm() {
    loop {
        Timer::after_millis(1500).await;

        if RUNNING.load(Ordering::Relaxed) {
            let side = CURRENT_SIDE_INDEX.load(Ordering::Relaxed);
            CURRENT_SIDE_INDEX.store((side + 1) % 2, Ordering::Relaxed);
        }
    }
}

async fn update_pwm () {
    let mode = CURRENT_MODE_INDEX.load(Ordering::Relaxed);
    let side = CURRENT_SIDE_INDEX.load(Ordering::Relaxed);
    let mut pwn_unlocked = PWM.lock().await;
    let mut pwn_config_unlocked = PWM_CONFIG.lock().await;
    if let (Some(pwm), Some(config)) = (pwn_unlocked.as_mut(), pwn_config_unlocked.as_mut()) {
        config.compare_b = MODES[mode][side];
        pwm.set_config(config);
    }
}