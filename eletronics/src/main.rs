#![no_std]
#![no_main]

extern crate alloc;

use core::time::Duration;

use alloc::format;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources, tcp::TcpSocket};
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Instant, Timer};
use embedded_io_async::Write as _;
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(feature = "esp32c3")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    rng::Rng,
    rtc_cntl::{
        Rtc,
        sleep::{RtcSleepConfig, TimerWakeupSource},
    },
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
        Client, MqttError,
        options::{ConnectOptions, PublicationOptions},
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttBinary, MqttString, QoS, TopicName},
};

esp_bootloader_esp_idf::esp_app_desc!();

type MqttClient = Client<'static, TcpSocket<'static>, AllocBuffer, 1, 1, 1>;

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
const PING_INTERVAL_SECS: &str = env!("PING_INTERVAL_SECS");

const TIMEOUT_US: u64 = 30_000;

static DISTANCE_SIGNAL: Signal<NoopRawMutex, Option<f32>> = Signal::new();
static MQTT_DONE: Signal<NoopRawMutex, ()> = Signal::new();

static mut MQTT_RX_BUF: [u8; 4096] = [0u8; 4096];
static mut MQTT_TX_BUF: [u8; 4096] = [0u8; 4096];

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    #[cfg(feature = "esp32")]
    esp_rtos::start(timg0.timer0);
    #[cfg(feature = "esp32c3")]
    {
        let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
        esp_rtos::start(timg0.timer0, sw_ints.software_interrupt0);
    }

    let mut trig = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let echo = Input::new(
        peripherals.GPIO18,
        InputConfig::default().with_pull(Pull::Down),
    );

    let stack = setup_wifi(&spawner, peripherals.WIFI).await;
    let mut rtc = Rtc::new(peripherals.LPWR);

    println!("--- Cat Water Fountain ---");

    loop {
        // Wait for WiFi link + DHCP
        while !stack.is_link_up() {
            Timer::after_millis(500).await;
        }
        println!("WiFi link up!");

        while !stack.is_config_up() {
            Timer::after_millis(100).await;
        }
        stack
            .config_v4()
            .inspect(|c| println!("IPv4 config: {c:?}"));

        // Reset signals
        DISTANCE_SIGNAL.reset();
        MQTT_DONE.reset();

        // Spawn MQTT task (runs independently — sends ping immediately)
        spawner.spawn(mqtt_task(stack)).ok();

        // Run sensor inline
        let distance = sensor_read(&mut trig, &echo).await;
        println!("Distance: {:?}", distance);

        // Signal distance to mqtt_task (unblocks its water-level publish)
        DISTANCE_SIGNAL.signal(distance);

        // Wait for MQTT task to finish
        MQTT_DONE.wait().await;

        // Light sleep
        light_sleep(&mut rtc);
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

#[embassy_executor::task]
async fn mqtt_task(stack: Stack<'static>) {
    match mqtt_run(stack).await {
        Ok(()) => println!("MQTT task completed"),
        Err(e) => println!("MQTT task failed: {:?}", e),
    }
    MQTT_DONE.signal(());
}

async fn mqtt_run(stack: Stack<'static>) -> Result<(), MqttError<'static>> {
    let mut client = create_mqtt_client(stack).await?;

    // Send ping immediately (independent of sensor)
    let ping_topic =
        unsafe { TopicName::new_unchecked(MqttString::from_slice("cat-water/ping").unwrap()) };
    let ping_opts = PublicationOptions {
        retain: false,
        topic: ping_topic.as_borrowed(),
        qos: QoS::AtMostOnce,
    };
    client
        .publish(&ping_opts, Bytes::Borrowed(b"ping"))
        .await?;
    println!("MQTT: ping sent");

    // Wait for sensor distance
    let distance = DISTANCE_SIGNAL.wait().await;

    if let Some(cm) = distance {
        let msg = format!("{:.1}", cm);
        let level_topic = unsafe {
            TopicName::new_unchecked(MqttString::from_slice("cat-water/water-level").unwrap())
        };
        let level_opts = PublicationOptions {
            retain: false,
            topic: level_topic.as_borrowed(),
            qos: QoS::AtMostOnce,
        };
        client
            .publish(&level_opts, Bytes::Borrowed(msg.as_bytes()))
            .await?;
        println!("MQTT: water-level sent: {}cm", msg);
    }

    Ok(())
}

async fn create_mqtt_client(stack: Stack<'static>) -> Result<MqttClient, MqttError<'static>> {
    let buffer = mk_static!(AllocBuffer, AllocBuffer);
    let mut client = Client::<'_, _, _, 1, 1, 1>::new(buffer);

    let connect_options = ConnectOptions {
        clean_start: true,
        keep_alive: KeepAlive::default(),
        session_expiry_interval: SessionExpiryInterval::NeverEnd,
        user_name: Some(MqttString::from_slice(MQTT_USER).unwrap()),
        password: Some(MqttBinary::from_slice(MQTT_PASSWORD.as_bytes()).unwrap()),
        will: None,
    };

    let rx_buf = unsafe { &mut *core::ptr::addr_of_mut!(MQTT_RX_BUF) };
    let tx_buf = unsafe { &mut *core::ptr::addr_of_mut!(MQTT_TX_BUF) };
    let mut socket = TcpSocket::new(stack, rx_buf, tx_buf);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    let host_port = MQTT_SERVER.strip_prefix("mqtt://").unwrap_or(MQTT_SERVER);
    let (host, port) = host_port.rsplit_once(':').unwrap_or((host_port, "1883"));
    let remote_ip: embassy_net::Ipv4Address = host.parse().unwrap();
    let remote_port: u16 = port.parse().unwrap();
    if let Err(e) = socket.connect((remote_ip, remote_port)).await {
        println!("MQTT TCP connect failed: {:?}", e);
        return Err(MqttError::Network(embedded_io_async::ErrorKind::Other));
    }

    client
        .connect(
            socket,
            &connect_options,
            Some(MqttString::from_slice("cat-water-fountain").unwrap()),
        )
        .await?;

    Ok(client)
}

async fn setup_wifi(
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

    spawner.spawn(connect_wifi(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    stack
}

#[embassy_executor::task]
async fn connect_wifi(mut controller: WifiController<'static>) {
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
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

fn light_sleep(rtc: &mut Rtc) {
    let interval: u64 = PING_INTERVAL_SECS.parse().unwrap();
    let timer = TimerWakeupSource::new(Duration::from_secs(interval));
    let cfg = RtcSleepConfig::light();
    rtc.sleep(&cfg, &[&timer]);
}
