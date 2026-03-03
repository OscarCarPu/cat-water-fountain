# Software 

Writen in bare metal rust with:
- `esp-hal`: hardware abstraction
- `esp-rtos` + `embassy-executor`: async runtime
- `embassy-time`: async delays
- `esp-bootloader-esp-idf`: bootloader
- `esp-backtrace` / `esp-println`: panic hanbdler + printing for debugging

## Target features 

- `esp32`: for the esp32 chip, developing 
- `esp32c3`: for the esp32c3 chip, production

## features

- [x]: reading from hc-sr04 
- []: ping system watchdog with mqtt 
- []: mqtt sending data to server
- []: sending info to motor with MOSFET
- []: reading cat distance from hc-sr04
- []: reading water level from hc-sr04
- []: deep sleep between reads 
