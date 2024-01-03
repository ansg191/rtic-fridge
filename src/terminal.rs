use core::fmt::Write;

use defmt::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{Deque, Vec};
use rtic::Mutex;
use stm32f0xx_hal::prelude::*;

use crate::{app::terminal::Context, ds18b20::Resolution};

pub const BUFFER_SIZE: usize = 32;
const OK_STR: &str = "ok\r\n";

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
pub fn terminal(mut cx: Context<'_>) {
    loop {
        let Some(line) = cx.shared.buffer.lock(get_line) else {
            return;
        };

        // Split line into arguments
        let mut args = line.split(|b| is_whitespace(*b));

        // Handle command
        match args.next() {
            None | Some(&[]) => trace!("Empty command"),
            Some(b"help") => print_uart(&mut cx, HELP_STR),
            Some(b"resolution") => match args.next() {
                None | Some(&[]) => match cx.shared.resolution.lock(|res| *res) {
                    Resolution::Bits9 => print_uart(&mut cx, "9\r\n"),
                    Resolution::Bits10 => print_uart(&mut cx, "10\r\n"),
                    Resolution::Bits11 => print_uart(&mut cx, "11\r\n"),
                    Resolution::Bits12 => print_uart(&mut cx, "12\r\n"),
                },
                Some(b"9") => {
                    cx.shared.resolution.lock(|res| *res = Resolution::Bits9);
                    print_uart(&mut cx, OK_STR);
                }
                Some(b"10") => {
                    cx.shared.resolution.lock(|res| *res = Resolution::Bits10);
                    print_uart(&mut cx, OK_STR);
                }
                Some(b"11") => {
                    cx.shared.resolution.lock(|res| *res = Resolution::Bits11);
                    print_uart(&mut cx, OK_STR);
                }
                Some(b"12") => {
                    cx.shared.resolution.lock(|res| *res = Resolution::Bits12);
                    print_uart(&mut cx, OK_STR);
                }
                Some(b) => unknown_argument(&mut cx, b),
            },
            Some(b"cooler") => match args.next() {
                None | Some(&[]) => {
                    if unwrap!(cx.shared.cooler.lock(|c| c.is_set_high())) {
                        print_uart(&mut cx, "on\r\n");
                    } else {
                        print_uart(&mut cx, "off\r\n");
                    }
                }
                Some(b"on") => {
                    unwrap!(cx.shared.cooler.lock(OutputPin::set_high));
                    print_uart(&mut cx, OK_STR);
                }
                Some(b"off") => {
                    unwrap!(cx.shared.cooler.lock(OutputPin::set_low));
                    print_uart(&mut cx, OK_STR);
                }
                Some(b) => unknown_argument(&mut cx, b),
            },
            Some(b"reset") => {
                print_uart(&mut cx, "Resetting...\r\n");
                cortex_m::peripheral::SCB::sys_reset();
            }
            Some(b) => {
                dbg!(b);
                print_uart(&mut cx, "Unknown command: '");
                // SAFETY: b may not be valid UTF-8, but we don't care cause we're just printing it
                // Also, including UTF8 checks would add a lot to the binary size
                print_uart(&mut cx, unsafe { core::str::from_utf8_unchecked(b) });
                print_uart(&mut cx, "'\r\n");
            }
        }
    }
}

fn get_line(buffer: &mut Deque<u8, BUFFER_SIZE>) -> Option<Vec<u8, BUFFER_SIZE>> {
    // Find newline
    let Some(idx) = buffer.iter().position(|b| is_newline(*b)) else {
        // No newline found
        return None;
    };

    // Pop line from buffer
    let mut line = Vec::<_, BUFFER_SIZE>::new();
    for _ in 0..=idx {
        // SAFETY: idx is guaranteed to be valid in buffer
        // line is guaranteed to be large enough to hold idx + 1 bytes
        unsafe {
            let b = buffer.pop_front_unchecked();
            line.push_unchecked(b);
        }
    }

    Some(line)
}

fn print_uart(cx: &mut Context, str: &str) {
    cx.shared.usart.lock(|tx| {
        if tx.write_str(str).is_err() {
            defmt::panic!("Failed to write to UART");
        }
    });
}

fn unknown_argument(cx: &mut Context, arg: &[u8]) {
    print_uart(cx, "Unknown argument: '");
    // SAFETY: b may not be valid UTF-8, but we don't care cause we're just printing it
    // Also, including UTF8 checks would add a lot to the binary size
    print_uart(cx, unsafe { core::str::from_utf8_unchecked(arg) });
    print_uart(cx, "'\r\n");
}

#[inline]
pub const fn is_newline(b: u8) -> bool {
    b == b'\n' || b == b'\r'
}

#[inline]
pub const fn is_whitespace(b: u8) -> bool {
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
