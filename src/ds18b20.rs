//! Implementation for the DS18B20 temperature sensor.

use core::convert::Infallible;

use defmt::Format;
use embedded_hal::blocking::delay::DelayUs;
use rtic_monotonics::stm32::{Tim2 as Mono, *};

use crate::{
    onewire::{crc::check_crc8, Address, Error, OneWire},
    thermometer::Temperature,
};

pub const CONVERT_T: u8 = 0x44;
pub const READ_SCRATCHPAD: u8 = 0xBE;
pub const WRITE_SCRATCHPAD: u8 = 0x4E;
pub const COPY_SCRATCHPAD: u8 = 0x48;
pub const RECALL_E2: u8 = 0xB8;

#[derive(Debug, Format, Copy, Clone, Eq, PartialEq)]
pub struct Ds18b20 {
    addr: Address,
}

impl Ds18b20 {
    #[inline]
    pub const fn new(addr: Address) -> Self {
        Self { addr }
    }

    fn read_scratchpad(
        &self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<[u8; 9], Error<Infallible>> {
        wire.send_command(Some(&self.addr), READ_SCRATCHPAD, delay)?;

        let mut buf = [0u8; 9];
        for x in &mut buf {
            *x = wire.read_byte(delay)?;
        }

        check_crc8(&buf)?;

        Ok(buf)
    }

    fn write_scratchpad(
        &mut self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
        data: [u8; 3],
    ) -> Result<(), Error<Infallible>> {
        wire.send_command(Some(&self.addr), WRITE_SCRATCHPAD, delay)?;
        wire.write_byte(data[0], delay)?;
        wire.write_byte(data[1], delay)?;
        wire.write_byte(data[2], delay)?;
        wire.reset(delay)?;
        Ok(())
    }

    /// Retrieves the resolution of the sensor
    pub fn resolution(
        &self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<Resolution, Error<Infallible>> {
        let buf = self.read_scratchpad(wire, delay)?;
        Resolution::from_config_register(buf[4]).ok_or(Error::UnexpectedResponse)
    }

    /// Sets the resolution of the sensor
    pub fn set_resolution(
        &mut self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
        res: Resolution,
    ) -> Result<(), Error<Infallible>> {
        let mut buf = self.read_scratchpad(wire, delay)?;
        buf[4] = res.to_config_register();
        self.write_scratchpad(wire, delay, [buf[2], buf[3], buf[4]])?;
        Ok(())
    }

    /// Starts a temperature conversion
    ///
    /// This will take some time, depending on the resolution of the sensor.
    ///
    /// Call [`Ds18b20::read_data`] to read the result after the conversion is done.
    pub fn start_measurement(
        &mut self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<(), Error<Infallible>> {
        wire.send_command(Some(&self.addr), CONVERT_T, delay)
    }

    /// Reads the temperature data from the sensor
    pub fn read_data(
        &self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<Temperature, Error<Infallible>> {
        let mut buf = self.read_scratchpad(wire, delay)?;

        let resolution =
            Resolution::from_config_register(buf[4]).ok_or(Error::UnexpectedResponse)?;

        match resolution {
            Resolution::Bits9 => buf[0] &= 0b1111_1000,
            Resolution::Bits10 => buf[0] &= 0b1111_1100,
            Resolution::Bits11 => buf[0] &= 0b1111_1110,
            Resolution::Bits12 => {}
        }

        let value = i16::from_le_bytes([buf[0], buf[1]]);
        Ok(Temperature::from_bits(i32::from(value)))
    }

    /// Asynchronously Measures the temperature
    ///
    /// Performs a temperature conversion, waits for the conversion to finish, and reads the result.
    pub async fn measure(
        &mut self,
        wire: &mut OneWire,
        delay: &mut impl DelayUs<u32>,
    ) -> Result<Temperature, Error<Infallible>> {
        let d = u64::from(self.resolution(wire, delay)?.conversion_time());
        self.start_measurement(wire, delay)?;

        Mono::delay(d.millis()).await;

        self.read_data(wire, delay)
    }
}

#[derive(Debug, Format, Copy, Clone, Eq, PartialEq)]
pub enum Resolution {
    Bits9,
    Bits10,
    Bits11,
    Bits12,
}

impl Resolution {
    fn from_config_register(reg: u8) -> Option<Resolution> {
        match reg {
            0b0001_1111 => Some(Resolution::Bits9),
            0b0011_1111 => Some(Resolution::Bits10),
            0b0101_1111 => Some(Resolution::Bits11),
            0b0111_1111 => Some(Resolution::Bits12),
            _ => None,
        }
    }

    pub fn to_config_register(self) -> u8 {
        match self {
            Resolution::Bits9 => 0b0001_1111,
            Resolution::Bits10 => 0b0011_1111,
            Resolution::Bits11 => 0b0101_1111,
            Resolution::Bits12 => 0b0111_1111,
        }
    }

    /// Returns the minimum conversion time in milliseconds
    pub fn conversion_time(self) -> u16 {
        match self {
            Resolution::Bits9 => 94,
            Resolution::Bits10 => 188,
            Resolution::Bits11 => 375,
            Resolution::Bits12 => 750,
        }
    }
}
