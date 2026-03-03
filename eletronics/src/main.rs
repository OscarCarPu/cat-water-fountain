#![no_std]
#![no_main]
use esp_bootloader_esp_idf as _;
esp_bootloader_esp_idf::esp_app_desc!();
use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;

const TIMEOUT_US: u64 = 30_000;

async fn read_distance(trig: &mut Output<'_>, echo: &Input<'_>) -> Option<f32> {
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

fn average_filter(values: &[f32; 10]) -> f32 {
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

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let mut trig = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let echo = Input::new(
        peripherals.GPIO18,
        InputConfig::default().with_pull(Pull::Down),
    );

    println!("--- Running Raw ---");
    led.set_low();

    let mut values = [0.0f32; 10];
    let mut count = 0usize;

    loop {
        match read_distance(&mut trig, &echo).await {
            Some(cm) => {
                values[count] = cm;
                count += 1;
            }
            None => {
                continue;
            }
        }

        Timer::after_millis(200).await;

        if count < 10 {
            continue;
        }

        let filtered = average_filter(&values);
        println!("Filtered: {}cm", filtered);

        if filtered < 20.0 {
            led.set_high();
        } else {
            led.set_low();
        }
        count = 0;
    }
}
