#![no_std]
#![no_main]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    timer::timg::TimerGroup,
};
use esp_println::println;

esp_bootloader_esp_idf::esp_app_desc!();

const TIMEOUT_US: u64 = 30_000;

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let mut trig = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let echo = Input::new(
        peripherals.GPIO18,
        InputConfig::default().with_pull(Pull::Down),
    );

    println!("--- Cat Water Fountain (sensor only) ---");

    loop {
        let distance = sensor_read(&mut trig, &echo).await;
        println!("Distance: {:?} cm", distance);
        Timer::after_millis(1000).await;
    }
}

async fn read_distance(trig: &mut Output<'_>, echo: &Input<'_>) -> Option<f32> {
    use esp_hal::delay::Delay;

    let delay = Delay::new();
    trig.set_high();
    delay.delay_micros(10);
    trig.set_low();

    let timeout = Instant::now();
    while echo.is_low() {
        if Instant::now().duration_since(timeout).as_micros() > TIMEOUT_US {
            return None;
        }
    }

    let start = Instant::now();
    while echo.is_high() {
        if Instant::now().duration_since(start).as_micros() > TIMEOUT_US {
            return None;
        }
    }

    let duration_us = Instant::now().duration_since(start).as_micros();
    Some(duration_us as f32 / 58.0)
}

fn median_filter(values: &[f32; 10]) -> f32 {
    let mut sorted = *values;
    for i in 1..sorted.len() {
        let mut j = i;
        while j > 0 && sorted[j - 1] > sorted[j] {
            sorted.swap(j - 1, j);
            j -= 1;
        }
    }
    let sum: f32 = sorted[2..8].iter().sum();
    sum / 6.0
}

async fn sensor_read(trig: &mut Output<'_>, echo: &Input<'_>) -> Option<f32> {
    let mut values = [0.0f32; 10];
    let mut count = 0usize;

    while count < 10 {
        match read_distance(trig, echo).await {
            Some(cm) => {
                values[count] = cm;
                count += 1;
            }
            None => {}
        }
        Timer::after_millis(500).await;
    }

    Some(median_filter(&values))
}
