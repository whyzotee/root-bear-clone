use esp_hal::{
    gpio::{AnyPin, Input, InputConfig, Pull},
    time::{Duration, Instant},
};

pub struct Button {
    input: Input<'static>,
    delay: u64,
    was_pressed: bool,
}

impl Button {
    pub fn new(pin: AnyPin<'static>) -> Self {
        Self {
            input: Input::new(pin, InputConfig::default().with_pull(Pull::Up)),
            delay: 20,
            was_pressed: false,
        }
    }

    pub fn just_preesed(&mut self) -> bool {
        let is_preesed = self.input.is_low();
        let edge = is_preesed && !self.was_pressed;

        self.was_pressed = is_preesed;

        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(self.delay) {}

        edge
    }

    pub fn is_held(&self) -> bool {
        self.input.is_low()
    }
}
