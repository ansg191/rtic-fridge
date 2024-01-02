#![feature(type_alias_impl_trait, lint_reasons)]
#![no_std]
#![no_main]
#![allow(dead_code)]

mod controller;
mod cooler;
// mod delay;
mod ds18b20;
mod onewire;
mod temp_controller;
mod terminal;
mod thermometer;

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = stm32f0xx_hal::pac, dispatchers = [USART1, TIM14])]
mod app {
    use defmt::{panic, *};
    use rtic_monotonics::{
        stm32::{Tim2 as Mono, *},
        Monotonic,
    };
    use stm32f0xx_hal::{
        delay::Delay,
        gpio::{
            gpioa::{PA15, PA2},
            Alternate, Output, Pin, PushPull, AF1,
        },
        pac::{Interrupt, IWDG, USART2},
        prelude::*,
        serial,
        serial::{Event, Serial},
        watchdog::Watchdog,
    };

    use crate::{
        controller::pid::PidController, cooler::PinCooler, onewire::OneWire, terminal::is_newline,
        thermometer::ds18b20::Ds18b20Thermometer,
    };

    #[shared]
    struct Shared {
        usart: Serial<USART2, PA2<Alternate<AF1>>, PA15<Alternate<AF1>>>,
        buffer: heapless::Deque<u8, { crate::terminal::BUFFER_SIZE }>,
        cooler: PinCooler<Pin<Output<PushPull>>>,
    }

    #[local]
    struct Local {
        ds18b20: Ds18b20Thermometer<Delay, 4>,
        pid: PidController,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        // Set system clock to 24 MHz
        let mut rcc = cx
            .device
            .RCC
            .configure()
            .hsi48()
            .sysclk(24.mhz())
            .pclk(24.mhz())
            .hclk(24.mhz())
            .freeze(&mut cx.device.FLASH);

        trace!("sysclk: {}", rcc.clocks.sysclk().0);
        trace!("hclk: {}", rcc.clocks.hclk().0);
        trace!("pclk: {}", rcc.clocks.pclk().0);

        // Enable tim2 monotonic
        let token = rtic_monotonics::create_stm32_tim2_monotonic_token!();
        Mono::start(24_000_000, token);

        // Setup systick delay
        let delay = Delay::new(cx.core.SYST, &rcc);

        // Setup GPIO
        let gpioa = cx.device.GPIOA.split(&mut rcc);
        let gpiob = cx.device.GPIOB.split(&mut rcc);
        let pb3 = gpiob.pb3.into_push_pull_output(&cx.cs);

        let _ = blinky::spawn(pb3.downgrade());
        let _ = watchdog::spawn(cx.device.IWDG);

        // Setup USART & USART interrupt
        let mut usart = Serial::usart2(
            cx.device.USART2,
            (
                gpioa.pa2.into_alternate_af1(&cx.cs),
                gpioa.pa15.into_alternate_af1(&cx.cs),
            ),
            115_200.bps(),
            &mut rcc,
        );
        usart.listen(Event::Rxne);
        rtic::pend(Interrupt::USART2);

        // Setup cooler
        let cooler = PinCooler::new(gpiob.pb4.into_push_pull_output(&cx.cs).downgrade());

        // Setup DS18B20
        let mut pa12 = gpioa.pa12.into_open_drain_output(&cx.cs);
        unwrap!(pa12.set_high());
        let wire = OneWire::new(pa12.downgrade());

        let mut ds18b20 = Ds18b20Thermometer::new(wire, delay);

        crate::temp_controller::add_devices(&mut ds18b20);

        // Setup PID
        let pid = crate::temp_controller::new_pid();

        // Launch temperature controller
        let _ = temp_controller::spawn();

        (
            Shared {
                // delay,
                usart,
                buffer: heapless::Deque::new(),
                cooler,
            },
            Local { ds18b20, pid },
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        rtic::pend(Interrupt::USART2);

        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(priority = 1)]
    async fn blinky(_: blinky::Context, mut pin: Pin<Output<PushPull>>) {
        unwrap!(pin.set_low());
        let mut now = Mono::now();
        loop {
            unwrap!(pin.toggle());
            now += 500.millis();
            Mono::delay_until(now).await;
        }
    }

    #[task(priority = 1)]
    async fn watchdog(_: watchdog::Context, wdg: IWDG) {
        let mut wdg = Watchdog::new(wdg);
        wdg.start(1.hz());

        loop {
            wdg.feed();
            Mono::delay(100.millis()).await;
        }
    }

    #[task(priority = 2, local = [ds18b20, pid], shared = [cooler])]
    async fn temp_controller(cx: temp_controller::Context) {
        crate::temp_controller::temp_controller(cx).await;
    }

    #[task(priority = 2, shared = [usart, buffer, cooler])]
    async fn terminal(cx: terminal::Context) {
        let usart = cx.shared.usart;
        let buffer = cx.shared.buffer;
        let cooler = cx.shared.cooler;

        (usart, buffer, cooler).lock(|usart, buffer, cooler| {
            crate::terminal::terminal(usart, buffer, cooler);
        });
    }

    #[task(binds = USART2, local = [times: u32 = 0], shared = [usart, buffer])]
    fn usart2(cx: usart2::Context) {
        *cx.local.times += 1;

        // Read & echo all available bytes from the usart
        (cx.shared.usart, cx.shared.buffer).lock(|usart, buffer| loop {
            match usart.read() {
                Ok(b) => {
                    // Echo back
                    if is_newline(b) {
                        let _ = nb::block!(usart.write(b'\r'));
                        let _ = nb::block!(usart.write(b'\n'));
                    } else {
                        let _ = nb::block!(usart.write(b));
                    }

                    // Append to buffer
                    if buffer.push_back(b).is_err() {
                        panic!("Buffer overflow");
                    }
                }
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(serial::Error::Framing)) => {
                    panic!("USART error: Framing")
                }
                Err(nb::Error::Other(serial::Error::Noise)) => panic!("USART error: Noise"),
                Err(nb::Error::Other(serial::Error::Overrun)) => {
                    panic!("USART error: Overrun")
                }
                Err(nb::Error::Other(serial::Error::Parity)) => {
                    panic!("USART error: Parity")
                }

                Err(nb::Error::Other(_)) => defmt::panic!("USART error: Unknown"),
                // Err(nb::Error::Other(e)) => core::panic!("USART error: {:?}", e),
            }
        });

        defmt::trace!("USART2 interrupt fired: {}", *cx.local.times);

        // Trigger terminal task to handle input
        let _ = terminal::spawn();
    }

    timestamp!("{=u64:us}", {
        Mono::now().duration_since_epoch().to_micros()
    });
}
