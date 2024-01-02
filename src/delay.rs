use embedded_hal::{blocking::delay::DelayUs, timer::CountDown};
use embedded_hal_1::delay::DelayNs;
use nb::block;
use stm32f0xx_hal::time::{Hertz, KiloHertz};
use void::ResultVoidExt;

pub struct Delay<T> {
    timer: T,
}

impl<T: CountDown<Time = Hertz>> Delay<T> {
    pub fn new(timer: T) -> Self {
        Self { timer }
    }
}

impl<T: CountDown<Time = Hertz>> DelayUs<u32> for Delay<T> {
    fn delay_us(&mut self, us: u32) {
        if us == 0 {
            return;
        }

        self.timer.start(Hertz(1_000_000 / us));
        block!(self.timer.wait()).void_unwrap();
    }
}

impl<T: CountDown<Time = Hertz>> DelayNs for Delay<T> {
    fn delay_ns(&mut self, ns: u32) {
        if ns == 0 {
            return;
        }

        self.timer.start(KiloHertz(1_000_000 / ns));
        block!(self.timer.wait()).void_unwrap();
    }
}
