# cat-water-fountain
Automated drinking fountain for cats. Designed to activate a water pump when a cat is detected nearby and publish water level to an MQTT broker — currently in development.

More detailed docs in `docs/`:
- [Software](docs/software.md)
- [Hardware](docs/hardware.md)

## Tech stack

Bare-metal Rust on an ESP32 (Xtensa LX6), no operating system:

- `esp-hal` — hardware abstraction
- `esp-rtos` + `embassy-executor` — async runtime
- `embassy-net` + `esp-radio` — TCP/IP and WiFi
- `rust-mqtt` — MQTT v5 client

## Electronics

| Component | Part |
|---|---|
| Microcontroller | ESP32 (Xtensa LX6) |
| Cat sensor | HC-SR04 ultrasonic |
| Water level sensor | HC-SR04 ultrasonic |
| Pump | USB submersible 5V (TUNFAN PT-100cm, 150L/H) |
| Pump switch | IRLZ44N logic-level MOSFET |
| Flyback protection | 1N4001 diode across pump |
| Power | 3.2V LiFePO4 (MCU) + 5V power bank (pump and sensors) |

## Architecture

Designed around two concurrent embassy tasks plus a main loop:

1. **`detect_cat_task`** — polls the cat HC-SR04 every 100 ms and writes the result to a `static AtomicBool CAT_PRESENT`. Runs independently so motor reaction time stays around 100 ms regardless of the slower main cycle.
2. **`maintain_wifi_connection` / `run_network_stack`** — keeps the WiFi link and embassy-net stack alive; reconnects on disconnect. *(planned)*
3. **Main loop** (every 5 s) — reads water level (10-sample median-filtered HC-SR04), reads `CAT_PRESENT`, switches the motor on/off via MOSFET, publishes water level to MQTT. *(planned)*

## Configuration

Copy `eletronics/.env.example` to `eletronics/.env` and fill in:

| Variable | Purpose |
|---|---|
| `WIFI_SSID` | WiFi network name |
| `WIFI_PASSWORD` | WiFi password |
| `MQTT_SERVER` | Broker URL — must be `mqtt://<ipv4>:<port>` (hostnames not yet supported) |
| `MQTT_USER` | MQTT username |
| `MQTT_PASSWORD` | MQTT password |

Credentials are compiled into the firmware at build time via `build.rs`.

## GPIO pin assignment

| Function | Pin |
|---|---|
| Cat sensor — HC-SR04 trig | GPIO5 |
| Cat sensor — HC-SR04 echo | GPIO18 (via 1kΩ/2kΩ divider) |
| Water sensor — HC-SR04 trig | GPIO19 |
| Water sensor — HC-SR04 echo | GPIO21 (via 1kΩ/2kΩ divider) |
| Motor MOSFET gate | GPIO22 |

## Build & flash

```sh
make esp32-build-dev    # compile
make esp32-flash-dev    # compile, flash, and open serial monitor on /dev/ttyUSB0
make esp32-stop-dev     # erase flash
```

Requires the Espressif toolchain (`~/export-esp.sh`) and `espflash`.

## MQTT topics *(planned)*

| Topic | Direction | Payload | Cadence |
|---|---|---|---|
| `cat-water/water-level` | publish | filtered distance in cm, e.g. `"7.3"` | every 5 s |
| `cat-water/heartbeat` | publish | uptime or ping | every 5 s |

## Feature status

- [x] Hardware and system design
- [x] HC-SR04 distance reading
- [ ] HC-SR04 water level measurement
- [ ] Motor control via MOSFET
- [ ] MVP with just motor activated on cat detected
- [ ] ml-drank tracking
- [ ] MQTT water level publishing
- [ ] Heartbeat MQTT
- [ ] Telegram alerts (low-water / no heartbeat)
- [ ] Polishing details and structure
