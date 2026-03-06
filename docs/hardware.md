# Materials

## Container

Plastic container food safe:
- 21.8cm (length) * 15.8cm (width) * 13.8cm (height)
- On the top part it has some borders making it 24cm (length) * 18cm (width)
- https://www.aliexpress.com/item/1005010183592949.html

## Pump

USB Submersible Aquarium Pump (TUNFAN Store):
- Model: PT-100cm (adjustable flow, no filtration)
- Voltage: 5V
- Power: 5W
- Flow rate: 150L/H
- Max lift: 100cm
- Cable length: 50cm
- Outlet nozzle diameter: 8.4mm
- Size: 2.8cm x 3.8cm x 3.3cm
- Material: Rubber, copper motor
- Features: Adjustable flow, USB powered, submersible, suction cup base, mesh debris filter
- Price: ~3.85 EUR

## Tubing

PVC Transparent Water Tube:
- Size: 8mm inner diameter x 10mm outer diameter
- Length: 1 meter
- Material: PVC plastic
- Price: ~1.72 EUR

## Microcontroller

ESP32-C3 SuperMini:
- Chip: ESP32-C3
- Features: WiFi, Bluetooth BLE
- Operating voltage: 3.3V
- TODO: add specific board link and detailed specs

## Sensors

### Ultrasonic Sensor - Cat Detection
- Model: HC-SR04 (or compatible)

### Ultrasonic Sensor - Water Level
- Model: HC-SR04 (or compatible)

## Power

### LiFePO4 Battery
- Voltage: 3.2V

### 5V Battery/Power Bank
- Requirements: 5V, 1A minimum (for pump and sensors)

## Electronic Components

### MOSFET
- Model: IRLZ44N (Logic Level MOSFET)
- Purpose: Pump switching control from ESP32

### Diode
- Model: 1N4001
- Purpose: Flyback protection, parallel to pump

### Capacitors
- 1000uF electrolytic - close to battery (power smoothing)
- 0.1uF ceramic - on motor terminals (noise filtering)

### Resistors
- 3kOhm x2 - Voltage divider for ultrasonic echo pins
- 2kOhm x2 - Voltage divider to GND for ultrasonic echo pins
- 1kOhm x2 - Voltage divider for ultrasonic echo pins
- 330Ohm x1 - MOSFET gate resistor
- 10kOhm x1 - MOSFET gate pull-down to GND

## 3D Printed Parts (PLA)

### Enclosure/Housing
- Purpose: Outer shell that encloses all components (container, pump, electronics)
- Material: PLA
- TODO: design and add STL files


## Other Materials

### Filtration
- Type: Active carbon or similar
- TODO: research and add specific product

### Ballast/Weight
- Purpose: Prevent container from tipping over
- TODO: determine material and specs
