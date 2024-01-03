use defmt::Format;

pub type Result<T, E> = core::result::Result<T, Error<E>>;

#[derive(Debug, Format, Copy, Clone)]
pub enum Error<E> {
    /// The Bus was expected to be pulled high by a ~5K ohm pull-up resistor, but it wasn't
    BusNotHigh,

    /// Pin Error
    Pin(E),

    /// An unexpected response was received from a command. This generally happens when a new sensor is added
    /// or removed from the bus during a command, such as a device search.
    UnexpectedResponse,

    FamilyCodeMismatch,
    CrcMismatch,
    Timeout,
}

impl<E> Error<E> {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BusNotHigh => "Bus not high",
            Self::Pin(_) => "Pin error",
            Self::UnexpectedResponse => "Unexpected response",
            Self::FamilyCodeMismatch => "Family code mismatch",
            Self::CrcMismatch => "CRC mismatch",
            Self::Timeout => "Timeout",
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(value: E) -> Self {
        Self::Pin(value)
    }
}
