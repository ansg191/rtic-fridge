use core::convert::Infallible;

use embedded_hal::blocking::delay::DelayUs;
use rtic_monotonics::stm32::{Tim2 as Mono, *};

use crate::{
    ds18b20::{Ds18b20, Resolution, CONVERT_T},
    onewire::{Address, Error, OneWire},
    thermometer::{Temperature, Thermometer},
};

pub struct Ds18b20Thermometer<D, const N: usize> {
    ow: OneWire,
    therms: heapless::Vec<Ds18b20, N>,
    resolution: Resolution,
    delay: D,
}

impl<D: DelayUs<u32>, const N: usize> Ds18b20Thermometer<D, N> {
    pub const fn new(ow: OneWire, delay: D) -> Self {
        Self {
            ow,
            therms: heapless::Vec::new(),
            resolution: Resolution::Bits12,
            delay,
        }
    }

    pub fn wire(&self) -> &OneWire {
        &self.ow
    }
    pub fn wire_mut(&mut self) -> &mut OneWire {
        &mut self.ow
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
    pub fn set_resolution(&mut self, resolution: Resolution) -> Result<(), Error<Infallible>> {
        self.resolution = resolution;

        for therm in self.therms.iter_mut() {
            therm.set_resolution(&mut self.ow, &mut self.delay, resolution)?;
        }

        Ok(())
    }

    pub fn add(&mut self, addr: crate::onewire::Address) -> Result<(), Error<Infallible>> {
        let mut therm = Ds18b20::new(addr);
        therm.set_resolution(&mut self.ow, &mut self.delay, self.resolution)?;

        if self.therms.push(Ds18b20::new(addr)).is_err() {
            defmt::panic!("Failed to add thermometer: OOM");
        }

        Ok(())
    }

    pub fn devices(&mut self) -> impl Iterator<Item = Result<Address, Error<Infallible>>> + '_ {
        self.ow.devices(&mut self.delay)
    }
}

impl<D: DelayUs<u32>, const N: usize> Thermometer for Ds18b20Thermometer<D, N> {
    type Error = Error<Infallible>;

    async fn read(&mut self) -> Result<Temperature, Self::Error> {
        let mut temps = heapless::Vec::<_, N>::new();

        // Start conversion of all thermometers simultaneously
        self.ow.send_command(None, CONVERT_T, &mut self.delay)?;

        // Wait for conversion to complete
        let delay = self.resolution.conversion_time();
        Mono::delay(u64::from(delay).millis()).await;

        for therm in self.therms.iter() {
            let temp = therm.read_data(&mut self.ow, &mut self.delay)?;
            unsafe {
                temps.push_unchecked(temp);
            }
        }

        if temps.is_empty() {
            Err(Error::Timeout)
        } else {
            Ok(temps.iter().sum::<Temperature>() / temps.len() as i32)
        }
    }
}
