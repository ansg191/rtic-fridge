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
        const LIMIT: Temperature = Temperature::const_from_int(100);

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

    async fn run(&mut self, temp: Temperature) -> Result<bool, Self::Error> {
        let output = self.pid.next_control_output(temp);
        Ok(output.output.is_negative())
    }
}
