.PHONY: esp32-build-dev esp32-build-prod esp32-build esp32-flash-dev esp32-flash-prod esp32-stop-dev esp32-stop-prod

esp32-build-dev:
	. ~/export-esp.sh && cd eletronics && cargo +esp build-devkit

esp32-build-prod:
	. ~/export-esp.sh && cd eletronics && cargo +esp build-prod

esp32-build: esp32-build-dev esp32-build-prod

esp32-flash-dev:
	. ~/export-esp.sh && cd eletronics && cargo +esp run-devkit

esp32-flash-prod:
	. ~/export-esp.sh && cd eletronics && cargo +esp run-prod

esp32-stop-dev:
	espflash erase-flash -p /dev/ttyUSB0

esp32-stop-prod:
	espflash erase-flash -p /dev/ttyUSB0
