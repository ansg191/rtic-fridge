use core::fmt::Write;

use defmt::*;
use heapless::{Deque, Vec};
use stm32f0xx_hal::{
    gpio::{Output, Pin, PushPull},
    prelude::*,
};

use crate::cooler::PinCooler;

pub const BUFFER_SIZE: usize = 32;

/// Terminal handler
///
/// Commands:
/// - `help` - Print help
/// - `devices` - List 1wire devices on the bus
/// - `resolution <9|10|11|12>?` - Get or set the resolution of the thermometers
/// - `pid` - Get the PID values
/// - `pid <kp> <ki> <kd>` - Set the PID values
/// - `temp` - Get the current temperature
/// - `cooler <on|off>?` - Turn the cooler on or off or get the current state
/// - `watch temp` - Watch temperature until `s` is pressed
/// - `dump temps` - Dump the temperature stored in flash
/// - `dump events` - Dump the events stored in flash
/// - `erase` - Erase the flash storage
/// - `reset` - Reset the MCU
#[cfg_attr(feature = "sizing", inline(never))]
pub fn terminal<W: Write>(
    tx: &mut W,
    buffer: &mut Deque<u8, BUFFER_SIZE>,
    cooler: &mut PinCooler<Pin<Output<PushPull>>>,
) {
    loop {
        // Find newline
        let Some(idx) = buffer.iter().position(|b| is_newline(*b)) else {
            // No newline found
            return;
        };

        // Pop line from buffer
        let mut line = Vec::<_, BUFFER_SIZE>::new();
        for _ in 0..(idx + 1) {
            // SAFETY: idx is guaranteed to be valid in buffer
            // line is guaranteed to be large enough to hold idx + 1 bytes
            unsafe {
                let b = buffer.pop_front_unchecked();
                line.push_unchecked(b);
            }
        }

        // Split line into arguments
        let mut args = line.split(|b| is_whitespace(*b));

        // Handle command
        match args.next() {
            None | Some(&[]) => trace!("Empty command"),
            Some(b"help") => print_uart(tx, HELP_STR),
            Some(b"cooler") => match args.next() {
                None | Some(&[]) => match unwrap!(cooler.is_set_high()) {
                    true => print_uart(tx, "on\r\n"),
                    false => print_uart(tx, "off\r\n"),
                },
                Some(b"on") => {
                    unwrap!(cooler.set_high());
                    print_uart(tx, "ok\r\n");
                }
                Some(b"off") => {
                    unwrap!(cooler.set_low());
                    print_uart(tx, "ok\r\n");
                }
                Some(b) => {
                    print_uart(tx, "Unknown argument: '");
                    print_uart(tx, unsafe { core::str::from_utf8_unchecked(b) });
                    print_uart(tx, "'\r\n");
                }
            },
            Some(b"reset") => {
                print_uart(tx, "Resetting...\r\n");
                cortex_m::peripheral::SCB::sys_reset();
            }
            Some(b) => {
                dbg!(b);
                print_uart(tx, "Unknown command: '");
                // SAFETY: b may not be valid UTF-8, but we don't care cause we're just printing it
                // Also, including UTF8 checks would add a lot to the binary size
                print_uart(tx, unsafe { core::str::from_utf8_unchecked(b) });
                print_uart(tx, "'\r\n");
            }
        }
    }
}

fn print_uart<W: Write>(tx: &mut W, str: &str) {
    match tx.write_str(str) {
        Ok(_) => {}
        Err(_) => defmt::panic!("Failed to write to UART"),
    }
}

#[inline]
pub fn is_newline(b: u8) -> bool {
    b == b'\n' || b == b'\r'
}

#[inline]
pub fn is_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\n' || b == b'\r' || b == b'\t'
}

const HELP_STR: &str = "Commands:\r
    help\r
    devices\r
    resolution <9|10|11|12>?\r
    pid\r
    pid <kp> <ki> <kd>\r
    temp\r
    cooler <on|off>?\r
    watch temp\r
    dump temps\r
    dump events\r
    erase\r
    reset\r
";
