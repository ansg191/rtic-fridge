# RTIC Fridge

A fridge built using an STM32F042K6 microcontroller running RTIC using a Thermoelectric cooler & DS18B20 thermometers.

![Text Size](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/ansg191/557b7b9cfe676e4097be5a69d354f42b/raw/badge.json)

## Usage

Run using `cargo run`.

Flash a microprocessor by running the following command:
```sh
cargo flash --connect-under-reset --chip STM32F042K6Tx --release
```
