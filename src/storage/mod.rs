use fixed::types::I6F2;
use heapless::HistoryBuffer;
use num_traits::AsPrimitive;
use rtic_monotonics::{stm32::Tim2 as Mono, Monotonic};
use rtic_sync::channel::{Sender, TrySendError};

use crate::thermometer::Temperature;

pub const CHAN_SIZE: usize = 1;

pub struct Storage<const N: usize> {
    temps: HistoryBuffer<StoredTemp, N>,
    tx: Sender<'static, StoredTemp, CHAN_SIZE>,
}

impl<const N: usize> Storage<N> {
    pub const fn new(tx: Sender<'static, StoredTemp, CHAN_SIZE>) -> Self {
        Self {
            temps: HistoryBuffer::new(),
            tx,
        }
    }

    pub fn write(&mut self, temp: Temperature) {
        let temp = StoredTemp::now_from_temp(temp);
        self.temps.write(temp);

        match self.tx.try_send(temp) {
            Ok(()) | Err(TrySendError::Full(_)) => (),
            Err(TrySendError::NoReceiver(_)) => unreachable!("No receiver"),
        }
    }

    pub fn oldest(&self) -> OldestOrdered<'_, N> {
        OldestOrdered {
            iter: self.temps.oldest_ordered(),
        }
    }

    pub fn recent(&self) -> Option<StoredTemp> {
        self.temps.recent().copied()
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct StoredTemp {
    /// Seconds since startup (LSB u24)
    secs: [u8; 3],
    /// Reduced precision temperature
    value: I6F2,
}

static_assertions::assert_eq_size!(StoredTemp, u32);

impl StoredTemp {
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
        Self::new(secs.as_(), value)
    }

    #[inline]
    pub fn now_from_temp(temp: Temperature) -> Self {
        Self::now(temp.saturating_to_num())
    }

    #[inline]
    pub const fn secs(self) -> u32 {
        u32::from_le_bytes([self.secs[0], self.secs[1], self.secs[2], 0])
    }

    #[inline]
    pub fn value(self) -> Temperature {
        self.value.to_num()
    }
}

impl From<StoredTemp> for (u32, Temperature) {
    fn from(value: StoredTemp) -> Self {
        (value.secs(), value.value())
    }
}

#[derive(Clone)]
pub struct OldestOrdered<'a, const N: usize> {
    iter: heapless::OldestOrdered<'a, StoredTemp, N>,
}

impl<'a, const N: usize> Iterator for OldestOrdered<'a, N> {
    type Item = StoredTemp;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().copied()
    }
}
