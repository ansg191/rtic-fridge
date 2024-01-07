//! Temperature Controller task

use core::convert::Infallible;

use defmt::{unreachable, *};
use rtic::Mutex;
use rtic_monotonics::{
    stm32::{Tim2 as Mono, *},
    Monotonic,
};
use stm32f0xx_hal::{delay::Delay, prelude::*};

use crate::{
    controller::{pid::PidController, Controller},
    onewire::Error,
    storage::{EventCode, StoredEvent},
    thermometer::Temperature,
};

pub const TARGET_TEMP: Temperature = Temperature::const_from_int(5);
const KP: Temperature = Temperature::from_bits(1 << 4);
const KI: Temperature = Temperature::from_bits(1 << 2);
const KD: Temperature = Temperature::from_bits(1 << 1);

#[allow(clippy::needless_lifetimes, reason = "clippy bug")]
#[cfg_attr(feature = "sizing", inline(never))]
pub async fn temp_controller<'a>(
    mut cx: crate::app::temp_controller::Context<'a>,
    mut delay: Delay,
) {
    let mut now = Mono::now();

    let mut last_res = None;

    loop {
        let resolution = cx.shared.resolution.lock(|res| *res);
        if last_res != Some(resolution) {
            last_res = Some(resolution);
            if let Err(e) =
                cx.local
                    .water_temp
                    .set_resolution(cx.local.wire, &mut delay, resolution)
            {
                error!("Error setting resolution: {}", e);

                let event = StoredEvent::now(EventCode::TempSensorError, e.as_str());
                let _ = cx.local.e_tx.send(event).await;

                last_res = None;
            } else {
                let event =
                    StoredEvent::now(EventCode::TempSensorResolutionChanged, resolution.as_str());
                let _ = cx.local.e_tx.send(event).await;
            }
        }

        match temp_controller_inner(&mut cx, &mut delay).await {
            Ok(()) => {}
            Err(e) => {
                error!("Error: {}", e);

                let event = StoredEvent::now(EventCode::TempSensorError, e.as_str());
                let _ = cx.local.e_tx.send(event).await;
            }
        }

        now += 2.secs();
        Mono::delay_until(now).await;
    }
}

async fn temp_controller_inner<'a>(
    cx: &mut crate::app::temp_controller::Context<'a>,
    delay: &mut Delay,
) -> Result<(), Error<Infallible>> {
    let temp = cx.local.water_temp.measure(cx.local.wire, delay).await?;

    let cooler_on = cx
        .local
        .pid
        .run(temp)
        .await
        .unwrap_or_else(|_e| unreachable!("PID error"));

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

    if cx.local.tx.send(temp).await.is_err() {
        unreachable!("Receiver dropped");
    }

    Ok(())
}

pub fn new_pid() -> PidController {
    PidController::new(TARGET_TEMP, KP, KI, KD)
}
