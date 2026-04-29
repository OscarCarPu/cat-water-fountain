#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources, tcp::TcpSocket};
use embassy_time::{Instant, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_radio::{
    Controller,
    wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState},
};
use rust_mqtt::{
    Bytes,
    buffer::AllocBuffer,
    client::{
        Client,
        options::{ConnectOptions, PublicationOptions},
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttBinary, MqttString, QoS, TopicName},
};

esp_bootloader_esp_idf::esp_app_desc!();

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_SERVER: &str = env!("MQTT_SERVER");
const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");

const HC_SR04_TIMEOUT_US: u64 = 30_000;
const HC_SR04_SAMPLES: usize = 10;
const HC_SR04_TRIM: usize = HC_SR04_SAMPLES / 5;

const CAT_DETECTION_THRESHOLD_CM: f32 = 25.0;
const CAT_POLL_INTERVAL_MS: u64 = 100;
const MAIN_LOOP_INTERVAL_MS: u64 = 5_000;

static CAT_PRESENT: AtomicBool = AtomicBool::new(false);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let cat_sensor = HcSr04::new(
        Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default()),
        Input::new(
            peripherals.GPIO18,
            InputConfig::default().with_pull(Pull::Down),
        ),
    );
    let mut water_sensor = HcSr04::new(
        Output::new(peripherals.GPIO19, Level::Low, OutputConfig::default()),
        Input::new(
            peripherals.GPIO21,
            InputConfig::default().with_pull(Pull::Down),
        ),
    );
    let mut motor = Motor::new(Output::new(
        peripherals.GPIO22,
        Level::Low,
        OutputConfig::default(),
    ));

    let mqtt = mk_static!(MqttResources, MqttResources::new());

    let stack = setup_wifi_stack(&spawner, peripherals.WIFI).await;
    wait_for_wifi_ready(stack).await;

    spawner.spawn(detect_cat_task(cat_sensor)).ok();

    println!("--- Cat Water Fountain ---");

    loop {
        let water_level_cm = read_water_level_cm(&mut water_sensor).await;
        println!("Water level: {:?} cm", water_level_cm);

        if CAT_PRESENT.load(Ordering::Relaxed) {
            motor.on();
        } else {
            motor.off();
        }

        publish_water_level(stack, water_level_cm, mqtt).await;

        Timer::after_millis(MAIN_LOOP_INTERVAL_MS).await;
    }
}

// ============================================================
// HC-SR04 driver
// ============================================================

struct HcSr04<'a> {
    trig: Output<'a>,
    echo: Input<'a>,
}

impl<'a> HcSr04<'a> {
    fn new(trig: Output<'a>, echo: Input<'a>) -> Self {
        Self { trig, echo }
    }

    async fn take_single_measurement_cm(&mut self) -> Option<f32> {
        use esp_hal::delay::Delay;

        let delay = Delay::new();
        self.trig.set_high();
        delay.delay_micros(10);
        self.trig.set_low();

        let timeout = Instant::now();
        while self.echo.is_low() {
            if Instant::now().duration_since(timeout).as_micros() > HC_SR04_TIMEOUT_US {
                return None;
            }
        }

        let echo_start = Instant::now();
        while self.echo.is_high() {
            if Instant::now().duration_since(echo_start).as_micros() > HC_SR04_TIMEOUT_US {
                return None;
            }
        }

        let pulse_us = Instant::now().duration_since(echo_start).as_micros();
        Some(pulse_us as f32 / 58.0)
    }

    async fn read_filtered_distance_cm(&mut self) -> Option<f32> {
        let mut samples = [0.0f32; HC_SR04_SAMPLES];
        let mut collected = 0usize;
        while collected < HC_SR04_SAMPLES {
            if let Some(cm) = self.take_single_measurement_cm().await {
                samples[collected] = cm;
                collected += 1;
            }
            Timer::after_millis(500).await;
        }
        Some(clean_distance_samples(&samples))
    }
}

fn clean_distance_samples(samples: &[f32; HC_SR04_SAMPLES]) -> f32 {
    let mut sorted = *samples;
    for i in 1..sorted.len() {
        let mut j = i;
        while j > 0 && sorted[j - 1] > sorted[j] {
            sorted.swap(j - 1, j);
            j -= 1;
        }
    }
    let middle = &sorted[HC_SR04_TRIM..HC_SR04_SAMPLES - HC_SR04_TRIM];
    let sum: f32 = middle.iter().sum();
    sum / middle.len() as f32
}

// ============================================================
// Cat sensor
// ============================================================

async fn read_cat_distance_cm(sensor: &mut HcSr04<'_>) -> Option<f32> {
    sensor.take_single_measurement_cm().await
}

#[embassy_executor::task]
async fn detect_cat_task(mut sensor: HcSr04<'static>) {
    loop {
        if let Some(cm) = read_cat_distance_cm(&mut sensor).await {
            CAT_PRESENT.store(cm < CAT_DETECTION_THRESHOLD_CM, Ordering::Relaxed);
        }
        Timer::after_millis(CAT_POLL_INTERVAL_MS).await;
    }
}

// ============================================================
// Water sensor
// ============================================================

async fn read_water_level_cm(sensor: &mut HcSr04<'_>) -> Option<f32> {
    sensor.read_filtered_distance_cm().await
}

// ============================================================
// Motor
// ============================================================

struct Motor<'a> {
    gate: Output<'a>,
}

impl<'a> Motor<'a> {
    fn new(gate: Output<'a>) -> Self {
        Self { gate }
    }

    fn on(&mut self) {
        self.gate.set_high();
        println!("Motor on");
    }

    fn off(&mut self) {
        self.gate.set_low();
        println!("Motor off");
    }
}

// ============================================================
// WiFi
// ============================================================

async fn setup_wifi_stack(
    spawner: &Spawner,
    wifi: esp_hal::peripherals::WIFI<'static>,
) -> Stack<'static> {
    let esp_radio_ctrl = &*mk_static!(Controller<'static>, esp_radio::init().unwrap());
    let (controller, interfaces) =
        esp_radio::wifi::new(esp_radio_ctrl, wifi, Default::default()).unwrap();
    let device = interfaces.sta;

    let net_config = embassy_net::Config::dhcpv4(Default::default());

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        device,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    Timer::after_millis(2000).await;
    spawner.spawn(maintain_wifi_connection(controller)).ok();
    spawner.spawn(run_network_stack(runner)).ok();

    stack
}

async fn wait_for_wifi_ready(stack: Stack<'static>) {
    while !stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    stack
        .config_v4()
        .inspect(|c| println!("IPv4 config: {c:?}"));
}

#[embassy_executor::task]
async fn maintain_wifi_connection(mut controller: WifiController<'static>) {
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_radio::wifi::sta_state() {
            WifiStaState::Connected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after_millis(5000).await;
            }
            _ => {}
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(WIFI_SSID.try_into().unwrap())
                    .with_password(WIFI_PASSWORD.try_into().unwrap()),
            );
            controller.set_config(&client_config).unwrap();
            println!("Starting WiFi...");
            controller.start_async().await.unwrap();
            println!("WiFi started!");
            controller
                .set_power_saving(esp_radio::wifi::PowerSaveMode::Maximum)
                .unwrap();
        }

        println!("Connecting to '{}'...", WIFI_SSID);
        match controller.connect_async().await {
            Ok(_) => println!("WiFi connected!"),
            Err(e) => {
                println!("WiFi connect failed: {:?}", e);
                Timer::after_millis(5000).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn run_network_stack(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

// ============================================================
// MQTT
// ============================================================

struct MqttResources {
    client_buffer: AllocBuffer,
    rx_buf: [u8; 4096],
    tx_buf: [u8; 4096],
}

impl MqttResources {
    fn new() -> Self {
        Self {
            client_buffer: AllocBuffer,
            rx_buf: [0u8; 4096],
            tx_buf: [0u8; 4096],
        }
    }
}

async fn publish_water_level(
    stack: Stack<'static>,
    water_level_cm: Option<f32>,
    mqtt: &mut MqttResources,
) {
    let Some(cm) = water_level_cm else {
        return;
    };

    let host_port = MQTT_SERVER.strip_prefix("mqtt://").unwrap_or(MQTT_SERVER);
    let (host, port) = host_port.rsplit_once(':').unwrap_or((host_port, "1883"));
    let remote_ip: embassy_net::Ipv4Address = host.parse().unwrap();
    let remote_port: u16 = port.parse().unwrap();

    let mut socket = TcpSocket::new(stack, &mut mqtt.rx_buf, &mut mqtt.tx_buf);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
    if let Err(e) = socket.connect((remote_ip, remote_port)).await {
        println!("MQTT TCP connect failed: {:?}", e);
        return;
    }

    let connect_options = ConnectOptions {
        clean_start: true,
        keep_alive: KeepAlive::default(),
        session_expiry_interval: SessionExpiryInterval::NeverEnd,
        user_name: Some(MqttString::from_slice(MQTT_USER).unwrap()),
        password: Some(MqttBinary::from_slice(MQTT_PASSWORD.as_bytes()).unwrap()),
        will: None,
    };
    let mut client = Client::<'_, _, _, 1, 1, 1>::new(&mut mqtt.client_buffer);
    if let Err(e) = client
        .connect(
            socket,
            &connect_options,
            Some(MqttString::from_slice("cat-water-fountain").unwrap()),
        )
        .await
    {
        println!("MQTT CONNECT failed: {:?}", e);
        return;
    }

    let mut buf = [0u8; 16];
    let len = format_decimal(&mut buf, cm);
    let topic =
        unsafe { TopicName::new_unchecked(MqttString::from_slice("cat-water/water-level").unwrap()) };
    let opts = PublicationOptions {
        retain: false,
        topic: topic.as_borrowed(),
        qos: QoS::AtMostOnce,
    };
    if let Err(e) = client.publish(&opts, Bytes::Borrowed(&buf[..len])).await {
        println!("MQTT publish failed: {:?}", e);
    }
}

fn format_decimal(buf: &mut [u8], value: f32) -> usize {
    use core::fmt::Write;
    struct W<'b> {
        buf: &'b mut [u8],
        pos: usize,
    }
    impl core::fmt::Write for W<'_> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            let end = self.pos + bytes.len();
            if end > self.buf.len() {
                return Err(core::fmt::Error);
            }
            self.buf[self.pos..end].copy_from_slice(bytes);
            self.pos = end;
            Ok(())
        }
    }
    let mut w = W { buf, pos: 0 };
    let _ = write!(w, "{:.1}", value);
    w.pos
}
