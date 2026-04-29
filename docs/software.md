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

Two concurrent embassy tasks plus the main loop:

1. **`maintain_wifi_connection` / `run_network_stack`** — keep the WiFi link and embassy-net stack alive; reconnect on disconnect.
2. **`detect_cat_task`** — fast cat detection. Polls the cat HC-SR04 every 100 ms with a single measurement and writes the boolean result to a `static AtomicBool CAT_PRESENT`.
3. **`main` loop** (every 5 s):
   - Read water level (10-sample median-filtered HC-SR04 read on the water sensor).
   - Read `CAT_PRESENT` and switch the motor on or off accordingly.
   - Publish the water level to MQTT.

Splitting cat detection into its own task keeps motor reaction time around 100 ms, independent of the slower water/MQTT cycle.

## Motor control

The motor (USB submersible pump) is gated by an IRLZ44N logic-level MOSFET driven from GPIO22.

- `Motor::on()` drives the gate high → pump runs.
- `Motor::off()` drives the gate low → 10kΩ gate pull-down keeps the MOSFET off.

The main loop calls `motor.on()` when `cat_distance_cm < 25.0` and `motor.off()` otherwise (or on sensor-read failure — fail-safe). The threshold is `CAT_DETECTION_THRESHOLD_CM` in `main.rs`.

## MQTT topics

| Topic | Direction | Payload | Cadence |
|---|---|---|---|
| `cat-water/water-level` | publish | filtered distance in cm, formatted as `%.1f` (e.g. `"7.3"`) | every 5 s |

The client connects, publishes, and disconnects each cycle; there is no persistent session.

Cat-presence events are not currently published — only the motor is driven from them locally. Adding a `cat-water/cat-present` topic is a future extension.

## Feature status

- [x] reading from hc-sr04
- [x] mqtt sending data to server (water level)
- [x] sending info to motor with MOSFET
- [x] reading cat distance from hc-sr04
- [x] reading water level from hc-sr04
- [ ] ml-drank tracking
- [ ] low-water Telegram alert
- [ ] ping/heartbeat topic
- [ ] deep sleep between reads (currently uses `Timer::after_millis` to pace the loop; light/deep sleep was removed because it disconnects WiFi)
