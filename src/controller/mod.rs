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
    /// Returns `true` if the cooler should be on, `false` otherwise.
    async fn run(&mut self, temp: Temperature) -> Result<bool, Self::Error>;
}
