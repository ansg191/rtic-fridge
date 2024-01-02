//! Temperature sensor interface

pub mod ds18b20;

use fixed::types::I28F4;

/// U28F4 is a fixed point number with 4 fractional bits and 28 integer bits.
/// This gives us a precision of 0.0625 degrees Celsius & a range of (-2^28, 2^28 - 0.0625).
pub type Temperature = I28F4;

pub trait Thermometer {
    type Error;

    /// Read the temperature in degrees Celsius
    ///
    /// U28F4 is a fixed point number with 4 fractional bits and 28 integer bits.
    /// This gives us a precision of 0.0625 degrees Celsius & a range of (-2^28, 2^28 - 0.0625).
    async fn read(&mut self) -> Result<Temperature, Self::Error>;
}

/// Fake thermometer for testing
#[cfg(feature = "fake")]
pub mod fake {
    use core::convert::Infallible;

    use crate::thermometer::{Temperature, Thermometer};

    /// A fake thermometer that always returns the same temperature
    pub struct FakeThermometer {
        temp: Temperature,
    }

    impl FakeThermometer {
        pub fn new(temp: impl Into<Temperature>) -> Self {
            Self { temp: temp.into() }
        }

        /// Get the current temperature
        pub fn temp(&self) -> Temperature {
            self.temp
        }
        /// Get a mutable reference to the current temperature
        pub fn temp_mut(&mut self) -> &mut Temperature {
            &mut self.temp
        }
    }

    impl Thermometer for FakeThermometer {
        type Error = Infallible;

        async fn read(&mut self) -> Result<Temperature, Self::Error> {
            Ok(self.temp)
        }
    }
}
