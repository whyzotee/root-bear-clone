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
    note_index: usize,
    note_started: Instant,
    note_active: bool,
    tone_playing: bool,
}

impl Sound {
    pub fn new(peripherals: AnyPin<'static>, ledc: LEDC<'static>) -> Self {
        Self {
            buzzer: peripherals,
            ledc: Ledc::new(ledc),
            note_index: 0,
            note_started: Instant::now(),
            note_active: false,
            tone_playing: false,
        }
    }

    /// Advances the title music one note at a time, leaving the main loop free
    /// to poll buttons and animate the display.
    pub fn play_menu_music(&mut self) {
        let (_, duration_ms) = MELODY[self.note_index];
        if self.note_active && self.note_started.elapsed() < Duration::from_millis(duration_ms) {
            return;
        }

        if self.tone_playing {
            self.mute();
        }

        if self.note_active {
            self.note_index = (self.note_index + 1) % MELODY.len();
        }
        self.note_active = true;

        let (note, _) = MELODY[self.note_index];
        if note == music::REST {
            self.note_started = Instant::now();
            return;
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
            .channel::<HighSpeed>(channel::Number::Channel0, self.buzzer.reborrow());

        channel0
            .configure(channel::config::Config {
                timer: &hstimer0,
                duty_pct: 50,
                drive_mode: DriveMode::PushPull,
            })
            .unwrap();
        self.tone_playing = true;
        self.note_started = Instant::now();
    }

    pub fn stop(&mut self) {
        if self.tone_playing {
            self.mute();
        }
        self.note_active = false;
        self.note_index = 0;
    }

    fn mute(&mut self) {
        // A freshly-created channel has no timer attached. Configure it with
        // zero duty before muting, otherwise set_duty returns Error::Channel.
        let mut hstimer0 = self.ledc.timer::<HighSpeed>(timer::Number::Timer0);
        hstimer0
            .configure(timer::config::Config {
                duty: timer::config::Duty::Duty10Bit,
                clock_source: timer::HSClockSource::APBClk,
                frequency: Rate::from_hz(440),
            })
            .unwrap();
        let channel0 = self
            .ledc
            .channel::<HighSpeed>(channel::Number::Channel0, self.buzzer.reborrow());
        let mut channel0 = channel0;
        channel0
            .configure(channel::config::Config {
                timer: &hstimer0,
                duty_pct: 0,
                drive_mode: DriveMode::PushPull,
            })
            .unwrap();
        self.tone_playing = false;
    }
}
