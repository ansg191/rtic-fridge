use core::fmt::Write;

use defmt::{panic, unreachable, *};
use embedded_hal::digital::v2::OutputPin;
use heapless::{Deque, Vec};
use num_traits::AsPrimitive;
use rtic::Mutex;
use stm32f0xx_hal::prelude::*;

use crate::{app::terminal::Context, ds18b20::Resolution, thermometer::Temperature};

pub const BUFFER_SIZE: usize = 32;
const OK_STR: &str = "<ok>\r\n";

const HELP_STR: &str = "Commands:\r
    help\r
    devices\r
    resolution <9|10|11|12>?\r
    pid\r
    pid <kp> <ki> <kd>\r
    temp\r
    cooler <on|off>?\r
    watch temps\r
    dump temps\r
    dump events\r
    erase\r
    reset\r
";

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
/// - `watch temps` - Watch temperature until `s` is pressed
/// - `dump temps` - Dump the temperature stored in flash
/// - `dump events` - Dump the events stored in flash
/// - `erase` - Erase the flash storage
/// - `reset` - Reset the MCU
#[cfg_attr(feature = "sizing", inline(never))]
pub async fn terminal(mut cx: Context<'_>) {
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
            Some(b"temp") => {
                let temp = cx.shared.storage.lock(|s| s.recent());
                if let Some(temp) = temp {
                    cx.shared.usart.lock(|tx| {
                        print_uint(tx, temp.secs());
                        print_uart_locked(tx, " ");
                        print_temp(tx, temp.value());
                        print_uart_locked(tx, "\r\n");
                    });
                } else {
                    print_uart(&mut cx, "<missing>\r\n");
                }
            }
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
            Some(b"watch") => match args.next() {
                None | Some(&[]) => print_uart(&mut cx, "Missing argument\r\n"),
                Some(b"temps") => watch_temps(&mut cx).await,
                Some(b) => unknown_argument(&mut cx, b),
            },
            Some(b"dump") => match args.next() {
                None | Some(&[]) => print_uart(&mut cx, "Missing argument\r\n"),
                Some(b"temps") => cx.shared.storage.lock(|storage| {
                    for temp in storage.oldest() {
                        cx.shared.usart.lock(|tx| {
                            print_uint(tx, temp.secs());
                            print_uart_locked(tx, " ");
                            print_temp(tx, temp.value());
                            print_uart_locked(tx, "\r\n");
                        });
                    }
                }),
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

#[inline]
pub const fn is_newline(b: u8) -> bool {
    b == b'\n' || b == b'\r'
}

#[inline]
pub const fn is_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\n' || b == b'\r' || b == b'\t'
}

fn print_uart(cx: &mut Context, str: &str) {
    cx.shared.usart.lock(|tx| print_uart_locked(tx, str));
}

fn print_uart_locked<W: Write>(tx: &mut W, str: &str) {
    if tx.write_str(str).is_err() {
        panic!("Failed to write to UART");
    }
}

fn unknown_argument(cx: &mut Context, arg: &[u8]) {
    cx.shared.usart.lock(|tx| {
        print_uart_locked(tx, "Unknown argument: '");
        // SAFETY: b may not be valid UTF-8, but we don't care cause we're just printing it
        // Also, including UTF8 checks would add a lot to the binary size
        print_uart_locked(tx, unsafe { core::str::from_utf8_unchecked(arg) });
        print_uart_locked(tx, "'\r\n");
    });
}

fn print_temp<W: Write>(tx: &mut W, temp: Temperature) {
    const FRAC_TOTAL: u16 = 10u16.pow(Temperature::FRAC_NBITS);

    let sign = temp.is_negative();

    let int_part = temp.to_bits().unsigned_abs() >> Temperature::FRAC_NBITS;
    let frac_part = temp.frac().to_bits().unsigned_abs();

    let mut total = 0;
    for i in 0..Temperature::FRAC_NBITS {
        let bit = (frac_part >> i) & 1;
        let value = FRAC_TOTAL / (1 << (Temperature::FRAC_NBITS - i));
        total += bit * value;
    }

    trace!(
        "int_part: {=u16}, total: {=u16}, sign: {=bool}",
        int_part,
        total,
        sign
    );

    if sign {
        print_uart_locked(tx, "-");
    }
    print_uint(tx, u32::from(int_part));
    print_uart_locked(tx, ".");
    print_uint(tx, u32::from(total));
}

fn print_uint<W: Write>(tx: &mut W, mut num: u32) {
    const BUF_SIZE: usize = 10;

    let mut buf = [0u8; BUF_SIZE];
    let mut idx = 0;

    loop {
        let digit: u8 = (num % 10).as_();
        num /= 10;

        buf[BUF_SIZE - idx - 1] = b'0' + digit;
        idx += 1;

        if num == 0 {
            break;
        }
    }

    let buf = &buf[BUF_SIZE - idx..];
    // SAFETY: buf is guaranteed to be valid ASCII
    print_uart_locked(tx, unsafe { core::str::from_utf8_unchecked(buf) });
}

/// Watch temperatures until 's' is pressed
async fn watch_temps(cx: &mut Context<'_>) {
    print_uart(cx, "Press 's' to stop watching\r\n");
    loop {
        // Wait for storage to re-send a temperature
        let Ok(temp) = cx.local.rx.recv().await else {
            unreachable!("Sender dropped")
        };

        // Print temperature to UART
        cx.shared.usart.lock(|tx| {
            print_uint(tx, temp.secs());
            print_uart_locked(tx, " ");
            print_temp(tx, temp.value());
            print_uart_locked(tx, "\r\n");
        });

        // Check if 's' is in the buffer and stop if it is
        // Also, clear the buffer to prevent it from overflowing
        let to_break = cx.shared.buffer.lock(|buffer| {
            let to_break = buffer.iter().any(|b| *b == b's');

            // Clear buffer
            buffer.clear();

            to_break
        });
        if to_break {
            break;
        }
    }
}
