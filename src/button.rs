use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

pub struct Button {
    input: Input<'static>,
    was_pressed: bool,
}

impl Button {
    pub fn new(pin: AnyPin<'static>) -> Self {
        Self {
            input: Input::new(pin, InputConfig::default().with_pull(Pull::Up)),
            was_pressed: false,
        }
    }

    pub fn just_preesed(&mut self) -> bool {
        let is_preesed = self.input.is_low();
        let edge = is_preesed && !self.was_pressed;
        self.was_pressed = is_preesed;
        edge
    }

    pub fn is_held(&self) -> bool {
        self.input.is_low()
    }
}
