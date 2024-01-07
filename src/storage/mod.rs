use fixed::types::I6F2;
use heapless::{HistoryBuffer, OldestOrdered};
use num_traits::AsPrimitive;
use rtic_monotonics::{stm32::Tim2 as Mono, Monotonic};
use rtic_sync::channel::{Sender, TrySendError};

use crate::thermometer::Temperature;

pub const CHAN_SIZE: usize = 1;

pub struct Storage<const N: usize, const E: usize> {
    temps: HistoryBuffer<StoredTemp, N>,
    events: HistoryBuffer<StoredEvent, E>,
    tx: Sender<'static, StoredTemp, CHAN_SIZE>,
}

impl<const N: usize, const E: usize> Storage<N, E> {
    pub const fn new(tx: Sender<'static, StoredTemp, CHAN_SIZE>) -> Self {
        Self {
            temps: HistoryBuffer::new(),
            events: HistoryBuffer::new(),
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
    pub fn write_event(&mut self, event: StoredEvent) {
        self.events.write(event);
    }

    pub fn temp_oldest(&self) -> OldestOrdered<'_, StoredTemp, N> {
        self.temps.oldest_ordered()
    }
    pub fn temp_recent(&self) -> Option<StoredTemp> {
        self.temps.recent().copied()
    }

    pub fn event_oldest(&self) -> OldestOrdered<'_, StoredEvent, E> {
        self.events.oldest_ordered()
    }
    pub fn event_recent(&self) -> Option<&StoredEvent> {
        self.events.recent()
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

#[derive(Debug, Clone)]
#[repr(C)]
pub struct StoredEvent {
    /// Seconds since startup (LSB u24)
    secs: [u8; 3],
    /// Event code
    pub code: EventCode,
    /// UTF-8 Null-terminated message
    msg: [u8; 12],
}

static_assertions::assert_eq_size!(StoredEvent, [u8; 16]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum EventCode {
    /// Unknown event type
    Unknown = 0,
    /// Temperature sensor error
    TempSensorError,
    /// Temperature sensor resolution changed
    TempSensorResolutionChanged,
    /// PID controller error
    PidError,
    /// PID controller target changed
    PidTargetChanged,
    /// PID parameters changed
    PidParamsChanged,
}

impl StoredEvent {
    pub fn new(secs: u32, code: EventCode, msg: [u8; 12]) -> Self {
        Self {
            secs: secs.to_le_bytes()[..3].try_into().unwrap(),
            code,
            msg,
        }
    }

    pub fn new_str(secs: u32, code: EventCode, msg: &str) -> Self {
        let mut bytes = [0u8; 12];

        let len = msg.len();
        if len >= 12 {
            // Truncate the message
            bytes[..12].copy_from_slice(&msg.as_bytes()[..12]);
        } else {
            // Copy the message and null-terminate
            bytes[..len].copy_from_slice(msg.as_bytes());
            bytes[len] = 0;
        }

        Self::new(secs, code, bytes)
    }

    pub fn now(code: EventCode, msg: &str) -> Self {
        let secs = Mono::now().duration_since_epoch().to_secs();
        Self::new_str(secs.as_(), code, msg)
    }

    #[inline]
    pub const fn secs(&self) -> u32 {
        u32::from_le_bytes([self.secs[0], self.secs[1], self.secs[2], 0])
    }

    pub fn msg(&self) -> &str {
        let len = self.msg.iter().position(|&b| b == 0).unwrap_or(12);
        // SAFETY: The message is always valid UTF-8
        unsafe { core::str::from_utf8_unchecked(&self.msg[..len]) }
    }
}

impl EventCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::TempSensorError => "Temperature sensor error",
            Self::TempSensorResolutionChanged => "Temperature sensor resolution changed",
            Self::PidError => "PID controller error",
            Self::PidTargetChanged => "PID controller target changed",
            Self::PidParamsChanged => "PID parameters changed",
        }
    }
}
