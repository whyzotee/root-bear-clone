use esp_hal::{
    gpio::{AnyPin, DriveMode},
    ledc::{
        HighSpeed, Ledc,
        channel::{self, ChannelIFace},
        timer::{self, TimerIFace},
    },
    peripherals::LEDC,
    time::{Duration, Instant, Rate},
};

use crate::music::{self, MELODY};

pub struct Sound {
    buzzer: AnyPin<'static>,
    ledc: Ledc<'static>,
}

impl Sound {
    pub fn new(peripherals: AnyPin<'static>, ledc: LEDC<'static>) -> Self {
        Self {
            buzzer: peripherals,
            ledc: Ledc::new(ledc),
        }
    }

    pub fn play_sound(&mut self) {
        for &(note, duration_ms) in MELODY.iter() {
            let note_duration = duration_ms;

            if note == music::REST {
                Self::blocking_delay(Duration::from_millis(note_duration));
                continue;
            }

            let freq = Rate::from_hz(note as u32);

            let mut hstimer0 = self.ledc.timer::<HighSpeed>(timer::Number::Timer0);

            hstimer0
                .configure(timer::config::Config {
                    duty: timer::config::Duty::Duty10Bit,
                    clock_source: timer::HSClockSource::APBClk,
                    frequency: freq,
                })
                .unwrap();

            let mut channel0 = self
                .ledc
                .channel(channel::Number::Channel0, self.buzzer.reborrow());

            channel0
                .configure(channel::config::Config {
                    timer: &hstimer0,
                    duty_pct: 50,
                    drive_mode: DriveMode::PushPull,
                })
                .unwrap();

            // ไฟล์นี้มี REST แยกไว้อยู่แล้ว
            // จึงเล่นเต็ม duration ได้เลย
            Self::blocking_delay(Duration::from_millis(note_duration));

            channel0.set_duty(0).unwrap();
        }
    }

    fn blocking_delay(duration: Duration) {
        let delay_start = Instant::now();
        while delay_start.elapsed() < duration {}
    }
}
