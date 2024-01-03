mod address;
pub mod commands;
pub mod crc;
mod error;

use core::convert::Infallible;

use embedded_hal::blocking::delay::DelayUs;
use stm32f0xx_hal::{
    gpio::{OpenDrain, Output, Pin},
    prelude::*,
};

pub use self::{address::Address, error::*};

pub struct OneWire {
    pin: Pin<Output<OpenDrain>>,
}

impl OneWire {
    pub fn new(pin: Pin<Output<OpenDrain>>) -> Self {
        Self { pin }
    }

    /// Perform a reset initialization sequence
    pub fn reset(&mut self, delay: &mut impl DelayUs<u32>) -> Result<(), Infallible> {
        // Wait for the bus to be pulled high by the pull-up resistor
        let mut retries = 125;
        while self.pin.is_low()? {
            if retries == 0 {
                return Err(Error::BusNotHigh);
            }
            retries -= 1;
            delay.delay_us(2);
        }

        // Pull the bus low for 480us
        self.pin.set_low()?;
        delay.delay_us(480);

        // Release the bus
        self.pin.set_high()?;
        delay.delay_us(70);

        // Read the bus
        let is_low = self.pin.is_low()?;
        delay.delay_us(410);

        if is_low {
            Ok(())
        } else {
            Err(Error::UnexpectedResponse)
        }
    }

    /// Write a single bit to the bus
    pub fn write_bit(
        &mut self,
        bit: bool,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        if bit {
            // Write a 1

            // Pull the bus low for 10us
            self.pin.set_low()?;
            delay.delay_us(10);

            // Release the bus
            self.pin.set_high()?;

            // Wait for the end of the timeslot
            delay.delay_us(55);
        } else {
            // Write a 0

            // Pull the bus low for 65us
            self.pin.set_low()?;
            delay.delay_us(65);

            // Release the bus
            self.pin.set_high()?;

            // Wait for the end of the timeslot
            delay.delay_us(5);
        }

        Ok(())
    }

    /// Read a single bit from the bus
    pub fn read_bit(&mut self, delay: &mut impl DelayUs<u32>) -> Result<bool, Infallible> {
        let ret = cortex_m::interrupt::free(|_| {
            // Pull the bus low for 3us
            self.pin.set_low()?;
            delay.delay_us(1);

            // Release the bus
            self.pin.set_high()?;

            // Wait 6us for devices to write
            delay.delay_us(1);

            // Read the bus
            self.pin.is_high()
        })?;

        // Wait for the end of the timeslot
        delay.delay_us(53);

        Ok(ret)
    }

    /// Write a single byte to the bus
    pub fn write_byte(
        &mut self,
        byte: u8,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        for i in 0..8 {
            self.write_bit((byte >> i) & 1 == 1, delay)?;
        }
        Ok(())
    }

    /// Write multiple bytes to the bus
    pub fn write_bytes(
        &mut self,
        bytes: &[u8],
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        for byte in bytes {
            self.write_byte(*byte, delay)?;
        }
        Ok(())
    }

    /// Read a single byte from the bus
    pub fn read_byte(&mut self, delay: &mut impl DelayUs<u32>) -> Result<u8, Infallible> {
        let mut ret = 0;
        for i in 0..8 {
            if self.read_bit(delay)? {
                ret |= 1 << i;
            }
        }
        Ok(ret)
    }

    /// Read multiple bytes from the bus
    pub fn read_bytes(
        &mut self,
        bytes: &mut [u8],
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        for byte in bytes {
            *byte = self.read_byte(delay)?;
        }
        Ok(())
    }

    /// Do a ROM select
    pub fn select_address(
        &mut self,
        device: &Address,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        self.write_byte(commands::MATCH_ROM, delay)?;
        self.write_bytes(&device.0.to_le_bytes(), delay)
    }

    /// Do a ROM skip
    pub fn skip_address(&mut self, delay: &mut impl DelayUs<u32>) -> Result<(), Infallible> {
        self.write_byte(commands::SKIP_ROM, delay)
    }

    /// Get iterator over all devices on the bus
    pub fn devices<'a, 'd, D: DelayUs<u32>>(
        &'a mut self,
        delay: &'d mut D,
    ) -> DeviceSearch<'a, 'd, D> {
        DeviceSearch {
            wire: self,
            last_discrepancy: 0,
            last_family_discrepancy: 0,
            last_device_flag: false,
            rom_no: [0; 8],
            delay,
        }
    }

    /// Send a command to the bus
    ///
    /// Does the following sequence:
    /// 1. Reset the bus
    /// 2. Select the given address, or skip if None
    /// 3. Write the command byte
    pub fn send_command(
        &mut self,
        address: Option<&Address>,
        command: u8,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Infallible> {
        self.reset(delay)?;
        if let Some(address) = address {
            self.select_address(address, delay)?;
        } else {
            self.skip_address(delay)?;
        }
        self.write_byte(command, delay)?;
        Ok(())
    }
}

pub struct DeviceSearch<'a, 'd, D> {
    wire: &'a mut OneWire,
    last_discrepancy: u8,
    last_family_discrepancy: u8,
    last_device_flag: bool,
    rom_no: [u8; 8],
    delay: &'d mut D,
}

impl<D: DelayUs<u32>> DeviceSearch<'_, '_, D> {
    pub fn search(&mut self) -> Result<Option<Address>, Infallible> {
        let mut id_bit_number = 1u8;
        let mut last_zero = 0u8;
        let mut rom_byte_number = 0u8;
        let mut rom_byte_mask = 1u8;
        let mut search_result = false;

        if !self.last_device_flag {
            self.wire.reset(self.delay)?;

            // Normal search
            self.wire.write_byte(commands::SEARCH_NORMAL, self.delay)?;

            // Loop to do the search
            while rom_byte_number < 8 {
                let id_bit = self.wire.read_bit(self.delay)?;
                let cmp_id_bit = self.wire.read_bit(self.delay)?;

                // Check for no devices on the bus
                if id_bit && cmp_id_bit {
                    break;
                }

                // All coupled devices have 0 or 1
                let search_direction = if id_bit != cmp_id_bit {
                    // Bit write value for search
                    id_bit
                } else {
                    // If this discrepancy if before the Last Discrepancy
                    // on a previous next then pick the same as last time
                    let sd = if id_bit_number < self.last_discrepancy {
                        (self.rom_no[rom_byte_number as usize] & rom_byte_mask) > 0
                    } else {
                        // If equal to last pick 1, if not then pick 0
                        id_bit_number == self.last_discrepancy
                    };

                    // If 0 was picked then record its position in LastZero
                    if !sd {
                        last_zero = id_bit_number;

                        // Check for Last discrepancy in family
                        if last_zero < 9 {
                            self.last_family_discrepancy = last_zero;
                        }
                    }

                    sd
                };

                // Set or clear the bit in the ROM byte rom_byte_number
                // with mask rom_byte_mask
                if search_direction {
                    self.rom_no[rom_byte_number as usize] |= rom_byte_mask;
                } else {
                    self.rom_no[rom_byte_number as usize] &= !rom_byte_mask;
                }

                // Serial number search direction write bit
                self.wire.write_bit(search_direction, self.delay)?;

                // Increment the byte counter id_bit_number
                // and shift the mask rom_byte_mask
                id_bit_number += 1;
                rom_byte_mask <<= 1;

                // If the mask is 0 then go to new SerialNum byte rom_byte_number and reset mask
                if rom_byte_mask == 0 {
                    rom_byte_number += 1;
                    rom_byte_mask = 1;
                }
            }

            // If the search was successful then
            if id_bit_number >= 65 {
                // Search successful so set LastDiscrepancy,LastDeviceFlag,search_result
                self.last_discrepancy = last_zero;

                // Check for last device
                if self.last_discrepancy == 0 {
                    self.last_device_flag = true;
                }
                search_result = true;
            }
        }

        if !search_result || self.rom_no[0] == 0 {
            self.last_discrepancy = 0;
            self.last_device_flag = false;
            Ok(None)
        } else {
            let address = Address(u64::from_le_bytes(self.rom_no));
            Ok(Some(address))
        }
    }
}

impl<D: DelayUs<u32>> Iterator for DeviceSearch<'_, '_, D> {
    type Item = Result<Address, Infallible>;

    fn next(&mut self) -> Option<Self::Item> {
        self.search().transpose()
    }
}
