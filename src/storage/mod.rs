use fixed::types::I6F2;
use heapless::HistoryBuffer;
use rtic_monotonics::{stm32::Tim2 as Mono, Monotonic};

use crate::thermometer::Temperature;

pub struct Storage<const N: usize> {
    temps: HistoryBuffer<Temp, N>,
}

impl<const N: usize> Storage<N> {
    pub const fn new() -> Self {
        Self {
            temps: HistoryBuffer::new(),
        }
    }

    pub fn write(&mut self, temp: Temperature) {
        let temp = Temp::now_from_temp(temp);
        self.temps.write(temp);
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct Temp {
    /// Seconds since startup (LSB u24)
    secs: [u8; 3],
    /// Reduced precision temperature
    value: I6F2,
}

static_assertions::assert_eq_size!(Temp, u32);

impl Temp {
    #[inline]
    fn new(secs: u32, value: I6F2) -> Self {
        Self {
            secs: secs.to_le_bytes()[..3].try_into().unwrap(),
            value,
        }
    }

    #[inline]
    fn now(value: I6F2) -> Self {
        let secs = Mono::now().duration_since_epoch().to_secs();
        Self::new(secs as u32, value)
    }

    #[inline]
    fn now_from_temp(temp: Temperature) -> Self {
        Self::now(temp.saturating_to_num())
    }
}
