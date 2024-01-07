# RTIC Fridge

A fridge built using an STM32F042K6 microcontroller running RTIC using a Thermoelectric cooler & DS18B20 thermometers.

## Usage

Run using `cargo run`.

Flash a microprocessor by running the following command:
```sh
cargo flash --connect-under-reset --chip STM32F042K6Tx --release
```
