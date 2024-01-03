#![feature(type_alias_impl_trait, lint_reasons)]
#![no_std]
#![no_main]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(dead_code, clippy::module_name_repetitions, clippy::wildcard_imports)]

mod controller;
mod cooler;
// mod delay;
mod ds18b20;
mod onewire;
mod storage;
mod temp_controller;
mod terminal;
mod thermometer;

use defmt_rtt as _;
use panic_probe as _;

const WATER_TEMP_ADDR: onewire::Address = onewire::Address(0x05_00_00_0F_83_FB_60_28);

#[rtic::app(device = stm32f0xx_hal::pac, dispatchers = [USART1, TIM14])]
mod app {
    use defmt::{panic, unreachable, *};
    use rtic_monotonics::{
        stm32::{Tim2 as Mono, *},
        Monotonic,
    };
    use rtic_sync::{
        channel::{ReceiveError, Receiver, Sender},
        make_channel,
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
        controller::pid::PidController,
        cooler::PinCooler,
        ds18b20::{Ds18b20, Resolution},
        onewire::OneWire,
        storage::Storage,
        terminal::is_newline,
        thermometer::Temperature,
        WATER_TEMP_ADDR,
    };

    #[shared]
    struct Shared {
        usart: Serial<USART2, PA2<Alternate<AF1>>, PA15<Alternate<AF1>>>,
        buffer: heapless::Deque<u8, { crate::terminal::BUFFER_SIZE }>,
        cooler: PinCooler<Pin<Output<PushPull>>>,
        resolution: Resolution,
        storage: Storage<100>,
    }

    #[local]
    struct Local {
        // ds18b20: Ds18b20Thermometer<Delay, 4>,

        // Temperature Controller
        wire: OneWire,
        water_temp: Ds18b20,
        pid: PidController,
        tx: Sender<'static, Temperature, 1>,
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
        let mut delay = Delay::new(cx.core.SYST, &rcc);

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
        let mut wire = OneWire::new(pa12.downgrade());

        for device in wire.devices(&mut delay) {
            let device = unwrap!(device);
            info!("Found device: {}", device);
        }

        let water_temp = Ds18b20::new(WATER_TEMP_ADDR);

        // Setup PID
        let pid = crate::temp_controller::new_pid();

        // Launch temperature controller
        let _ = temp_controller::spawn(delay);

        // Setup channels
        let (tx, rx) = make_channel!(Temperature, 1);

        // Setup Storage
        let storage = Storage::new();

        // Launch storage task
        let _ = storage::spawn(rx);

        (
            Shared {
                // delay,
                usart,
                buffer: heapless::Deque::new(),
                cooler,
                resolution: Resolution::Bits12,
                storage,
            },
            Local {
                // ds18b20,
                wire,
                water_temp,
                pid,
                tx,
            },
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

    #[task(priority = 2, local = [wire, water_temp, pid, tx], shared = [cooler, resolution])]
    async fn temp_controller(cx: temp_controller::Context, delay: Delay) {
        crate::temp_controller::temp_controller(cx, delay).await;
    }

    #[task(priority = 1, shared = [storage])]
    async fn storage(mut cx: storage::Context, mut rx: Receiver<'static, Temperature, 1>) {
        loop {
            let temp = match rx.recv().await {
                Ok(temp) => temp,
                Err(ReceiveError::Empty) => continue,
                Err(ReceiveError::NoSender) => unreachable!("Sender dropped"),
            };

            cx.shared.storage.lock(|storage| {
                storage.write(temp);
            });
        }
    }

    #[task(priority = 2, shared = [usart, buffer, cooler, resolution, storage])]
    async fn terminal(cx: terminal::Context) {
        let usart = cx.shared.usart;
        let buffer = cx.shared.buffer;
        let cooler = cx.shared.cooler;
        let resolution = cx.shared.resolution;
        let storage = cx.shared.storage;

        (usart, buffer, cooler, resolution, storage).lock(
            |usart, buffer, cooler, resolution, storage| {
                crate::terminal::terminal(usart, buffer, cooler, resolution, storage);
            },
        );
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
                    panic!("USART error: Framing");
                }
                Err(nb::Error::Other(serial::Error::Noise)) => panic!("USART error: Noise"),
                Err(nb::Error::Other(serial::Error::Overrun)) => {
                    panic!("USART error: Overrun");
                }
                Err(nb::Error::Other(serial::Error::Parity)) => {
                    panic!("USART error: Parity");
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
