use core::convert::Infallible;

use pid::Pid;

use crate::thermometer::Temperature;

pub struct PidController {
    pid: Pid<Temperature>,
}

impl PidController {
    pub fn new(
        target: impl Into<Temperature>,
        kp: impl Into<Temperature>,
        ki: impl Into<Temperature>,
        kd: impl Into<Temperature>,
    ) -> Self {
        const LIMIT: Temperature = Temperature::const_from_int(128);

        let mut pid = Pid::new(target, LIMIT);
        pid.p(kp, LIMIT);
        pid.i(ki, LIMIT);
        pid.d(kd, LIMIT);

        Self { pid }
    }
}

impl super::Controller for PidController {
    type Error = Infallible;

    fn set_target(&mut self, target: Temperature) {
        self.pid.setpoint = target;
    }

    fn get_target(&self) -> Temperature {
        self.pid.setpoint
    }

    async fn run(&mut self, temp: Temperature) -> Result<u8, Self::Error> {
        let output = self.pid.next_control_output(temp);

        // Map output from range (-128, 128) to (0, 255)
        let output = output
            .output
            .saturating_add(Temperature::const_from_int(128))
            .saturating_to_num();
        Ok(output)
    }
}
