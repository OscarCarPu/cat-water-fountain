# cat-water-fountain
Drinking fountain for cats with tracking of ml drank and sensors for activating only when cat is close.

# Initial ideas 

## Hardware
- Water motor for pumping the water up
- 3d printed container and output of water from it 
- Ultrasonic Sensor for detecting if cat is close
- Esp32 for sending data
- water level ultrasonic
- Some exposy resin for coating the container FOOD SAFE
- Something heavy so the container doesn't fall 
- Filtration: active carbon? or something like it
- Air flow so there is no condensation for the sensors

## Software
- Keeping track of the water that its drank, a history of it.
- Sending data to the server each 5 min 
- When the cat is close, activate the water pump 
- WHen the water is low, send a message through telegram for filling the water 
- Alerts if the cats aren't drinking or similar things

## Electronics
- Esp32-c3 mini 3.2 V with a LiFePO4 battery
- HC-SR04 Ultrasonic x2
- USB Submersible Pump
- 5V batteryA 1A min

Others:
- Diodes as shields 
- resistors 
- Logic MOSFET

### Schema connection

**ESP32-C3**
3.2V -> ESP32-c3 

**Sensors to ESP32-C3**
Ultrasonic cat echo -> 3kΩ -> ESP32-c3 -> Ultrasonic cat trig
Ultrasonic water level echo -> 3kΩ -> ESP32-c3 -> Ultrasonic water level trig
ESP32-C3 -> 330Ω -> Logic MOSFET IRLZ44N -> pump 


**5V and GND**
Directly to the pump and sensors on parallel 
MOSFET -> 10kΩ -> GND

### Other things
Comon ground of the batteris

Between the echo pin of the ultrasonic and the input pin of the esp32:
Echo pin -> 1kΩ -> Input pin 
Input pin -> 2kΩ -> GND

A 1000uF capacitor as close to the battery

A 0.1uF capacitor on the motor terminals for noise

Diode between the mosfet and the pump  1N4001, parallel
