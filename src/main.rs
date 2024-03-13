#![no_std]
#![no_main]

use core::sync::atomic::AtomicU8;
use defmt::*;
use embassy_rp::{bind_interrupts, gpio:: {Pin, AnyPin, Level, Output, Pull}, adc, pwm, peripherals};
use embassy_rp::peripherals::PIN_27;
use embassy_rp::pwm::PwmPinB;
use embassy_time::Timer;
use fixed::traits::ToFixed;
use {defmt_rtt as _, panic_probe as _};

mod debouncer;

static MODES: [[u16; 2]; 3] = [
    [1000, 2000],
    [750, 2250],
    [500, 2500],
];
static CURRENT_MODE_INDEX: AtomicU8 = AtomicU8::new(0);

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
        info!("led on!");
        led.set_high();
        Timer::after_secs(1).await;

        info!("led off!");
        led.set_low();
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn run_adc(adc_pin: peripherals::PIN_26, p_adc: peripherals::ADC) {

    let mut adc = adc::Adc::new(p_adc, Irqs, adc::Config::default());

    let mut p26 = adc::Channel::new_pin(adc_pin, Pull::Up);

    loop {
        let level = adc.read(&mut p26).await.unwrap();
        info!("Pin 26 ADC: {}", level);
        Timer::after_secs(1).await;
    }
}

#[embassy_executor::task]
async fn run_pwm(pwm_pin: PIN_27, pwm_channel: peripherals::PWM_CH5) {

    let mut c: pwm::Config = Default::default();
    c.divider = 125.to_fixed();
    c.top = 20000;
    c.compare_b = 1500;
    let mut pwm = pwm::Pwm::new_output_b(pwm_channel, pwm_pin, c.clone());


    loop {
        info!("current LED duty cycle: {}", c.compare_b);
        Timer::after_millis(300).await;

        if c.compare_b < 2500 {
            c.compare_b += 100;
        } else {
            c.compare_b = 500;
        }
        pwm.set_config(&c);
    }
}


