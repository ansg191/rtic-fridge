//! Temperature sensor interface

use fixed::types::I12F4;

/// U12F4 is a fixed point number with 4 fractional bits and 12 integer bits.
/// This gives us a precision of 0.0625 degrees Celsius & a range of (-2^11, 2^11 - 0.0625).
pub type Temperature = I12F4;
