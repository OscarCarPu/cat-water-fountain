# Software

Written in bare metal Rust with:
- `esp-hal`: hardware abstraction
- `esp-rtos` + `embassy-executor`: async runtime
- `embassy-time`: async delays
- `embassy-net`: TCP/IP stack
- `esp-radio`: WiFi
- `rust-mqtt`: MQTT v5 client
- `esp-bootloader-esp-idf`: bootloader
- `esp-backtrace` / `esp-println`: panic handler + serial print

## Target

- `esp32` (Xtensa LX6): the only supported target. The previous `esp32c3` (XIAO/SuperMini) target has been removed.

Build/flash from the project root:
- `make esp32-build-dev` — compile
- `make esp32-flash-dev` — compile, flash, and open serial monitor on `/dev/ttyUSB0`
- `make esp32-stop-dev` — erase flash on the connected board

## Configuration

The firmware reads credentials at compile time from `eletronics/.env` (loaded by `build.rs`). Required keys:

| Variable | Purpose |
|---|---|
| `WIFI_SSID` | WiFi network name |
| `WIFI_PASSWORD` | WiFi network password |
| `MQTT_SERVER` | Broker URL — must be `mqtt://<ipv4>:<port>`, hostnames are not yet supported |
| `MQTT_USER` | MQTT username |
| `MQTT_PASSWORD` | MQTT password |

Copy `eletronics/.env.example` to `eletronics/.env` and fill it in before building.

## GPIO pin assignment

| Function | Pin |
|---|---|
| Cat sensor — HC-SR04 trig | GPIO5 |
| Cat sensor — HC-SR04 echo | GPIO18 (via 1kΩ/2kΩ divider) |
| Water sensor — HC-SR04 trig | GPIO19 |
| Water sensor — HC-SR04 echo | GPIO21 (via 1kΩ/2kΩ divider) |
| Motor MOSFET gate | GPIO22 |

## Architecture

Designed around two concurrent embassy tasks plus a main loop:

1. **`detect_cat_task`** — polls the cat HC-SR04 every 100 ms with a single measurement and writes the boolean result to a `static AtomicBool CAT_PRESENT`. Running it as its own task keeps motor reaction time around 100 ms, independent of the slower main cycle.
2. **`maintain_wifi_connection` / `run_network_stack`** — keep the WiFi link and embassy-net stack alive; reconnect on disconnect. *(planned)*
3. **`main` loop** (every 5 s) *(planned)*:
   - Read water level (10-sample median-filtered HC-SR04 read on the water sensor).
   - Read `CAT_PRESENT` and switch the motor on or off via MOSFET.
   - Publish the water level to MQTT.

## Motor control *(planned)*

The motor (USB submersible pump) will be gated by an IRLZ44N logic-level MOSFET driven from GPIO22.

- `Motor::on()` drives the gate high → pump runs.
- `Motor::off()` drives the gate low → 10kΩ gate pull-down keeps the MOSFET off.

The main loop will call `motor.on()` when `CAT_PRESENT` is true and `motor.off()` otherwise (fail-safe on sensor error). Detection threshold is `CAT_DETECTION_THRESHOLD_CM` in `main.rs`.

## MQTT topics *(planned)*

| Topic | Direction | Payload | Cadence |
|---|---|---|---|
| `cat-water/water-level` | publish | filtered distance in cm, formatted as `%.1f` (e.g. `"7.3"`) | every 5 s |
| `cat-water/heartbeat` | publish | uptime or ping | every 5 s |

The client will connect, publish, and disconnect each cycle; no persistent session.

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
