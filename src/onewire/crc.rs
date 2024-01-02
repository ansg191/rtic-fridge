use core::convert::Infallible;

use super::{Error, Result};

/// Calculates the crc8 of the input data.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0;
    for byte in data {
        let mut byte = *byte;
        for _ in 0..8 {
            let x = (byte ^ crc) & 0x01;
            crc >>= 1;
            if x != 0 {
                crc ^= 0x8C;
            }
            byte >>= 1;
        }
    }
    crc
}

/// Checks to see if data (including the crc byte) passes the crc check.
///
/// A nice property of this crc8 algorithm is that if you include the crc value in the data
/// it will always return 0, so it's not needed to separate the data from the crc value
pub fn check_crc8(data: &[u8]) -> Result<(), Infallible> {
    if crc8(data) == 0 {
        Ok(())
    } else {
        Err(Error::CrcMismatch)
    }
}
