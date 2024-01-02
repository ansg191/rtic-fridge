//! Temperature Controller task

use core::convert::Infallible;

use defmt::{panic, *};
use embedded_hal::blocking::delay::DelayUs;
use rtic::Mutex;
use rtic_monotonics::{
    stm32::{Tim2 as Mono, *},
    Monotonic,
};
use stm32f0xx_hal::prelude::*;

use crate::{
    controller::{pid::PidController, Controller},
    onewire::{Address, Error},
    thermometer::{ds18b20::Ds18b20Thermometer, Temperature, Thermometer},
};

pub const TARGET_TEMP: Temperature = Temperature::const_from_int(5);
const KP: Temperature = Temperature::from_bits(1 << 4);
const KI: Temperature = Temperature::from_bits(1 << 2);
const KD: Temperature = Temperature::from_bits(1 << 1);

#[allow(clippy::needless_lifetimes, reason = "clippy bug")]
#[cfg_attr(feature = "sizing", inline(never))]
pub async fn temp_controller<'a>(mut cx: crate::app::temp_controller::Context<'a>) {
    let mut now = Mono::now();

    loop {
        trace!("temp_controller");

        // TODO: Error handling
        match temp_controller_inner(&mut cx).await {
            Ok(_) => {}
            Err(e) => {
                error!("Error: {}", e);
            }
        }

        now += 2.secs();
        Mono::delay_until(now).await;
    }
}

async fn temp_controller_inner<'a>(
    cx: &mut crate::app::temp_controller::Context<'a>,
) -> Result<(), Error<Infallible>> {
    let temp = cx.local.ds18b20.read().await?;

    let cooler_on = cx.local.pid.run(temp).await.unwrap_or_else(|e| {
        error!("Error running PID controller: {}", e);
        false
    });

    debug!(
        "Temperature: {=f32}, Cooler: {=bool}",
        temp.to_num::<f32>(),
        cooler_on
    );

    cx.shared.cooler.lock(|cooler| {
        if cooler_on {
            cooler.set_high()
        } else {
            cooler.set_low()
        }
    })?;

    Ok(())
}

pub fn new_pid() -> PidController {
    PidController::new(TARGET_TEMP, KP, KI, KD)
}

pub fn add_devices<D: DelayUs<u32>, const N: usize>(therms: &mut Ds18b20Thermometer<D, N>) {
    trace!("add_devices");
    let mut addrs = heapless::Vec::<Address, 4>::new();

    for addr in therms.devices() {
        let addr = unwrap!(addr);
        info!("Found device: {}", addr);

        if addrs.push(addr).is_err() {
            panic!("Failed to add device: OOM");
        }
    }

    for addr in addrs {
        unwrap!(therms.add(addr));
    }
}
