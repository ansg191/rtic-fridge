//! Controller to manage [`Cooler`] to keep a constant temperature.

use crate::thermometer::Temperature;

pub mod pid;

pub trait Controller {
    type Error;

    /// Set the target temperature in degrees Celsius
    fn set_target(&mut self, target: Temperature);

    /// Get the target temperature in degrees Celsius
    fn get_target(&self) -> Temperature;

    /// Run the controller for a single tick
    ///
    /// Returns 0 if cooler should be completely off, 255 if cooler should be completely on, or
    /// somewhere in between.
    async fn run(&mut self, temp: Temperature) -> Result<u8, Self::Error>;
}
