//! Temperature sensor interface

use fixed::types::I28F4;

/// U28F4 is a fixed point number with 4 fractional bits and 28 integer bits.
/// This gives us a precision of 0.0625 degrees Celsius & a range of (-2^28, 2^28 - 0.0625).
pub type Temperature = I28F4;
