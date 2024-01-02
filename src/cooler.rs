//! Thermo-electric cooler (TEC) driver.

use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};

/// Thermo-electric cooler (TEC) driver.
pub trait Cooler: StatefulOutputPin {}

/// A cooler that uses a GPIO pin.
pub struct PinCooler<PIN: StatefulOutputPin> {
    pin: PIN,
}

impl<PIN: StatefulOutputPin> PinCooler<PIN> {
    pub fn new(pin: PIN) -> Self {
        Self { pin }
    }
}

impl<PIN: StatefulOutputPin> OutputPin for PinCooler<PIN> {
    type Error = PIN::Error;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }
}

impl<PIN: StatefulOutputPin> StatefulOutputPin for PinCooler<PIN> {
    fn is_set_high(&self) -> Result<bool, Self::Error> {
        self.pin.is_set_high()
    }

    fn is_set_low(&self) -> Result<bool, Self::Error> {
        self.pin.is_set_low()
    }
}

impl<PIN: StatefulOutputPin> Cooler for PinCooler<PIN> {}
