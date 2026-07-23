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

use crate::{
    game::ScoreResult,
    music::{self, MELODY},
};

pub struct Sound {
    buzzer: AnyPin<'static>,
    ledc: Ledc<'static>,
    note_index: usize,
    note_started: Instant,
    note_active: bool,
    tone_playing: bool,
    pour_pitch_started: Instant,
    pour_pitch_high: bool,
    effect_notes: [(u32, u64); 3],
    effect_len: usize,
    effect_index: usize,
    effect_started: Instant,
    effect_active: bool,
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
            pour_pitch_started: Instant::now(),
            pour_pitch_high: false,
            effect_notes: [(0, 0); 3],
            effect_len: 0,
            effect_index: 0,
            effect_started: Instant::now(),
            effect_active: false,
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
        self.effect_active = false;
    }

    /// A soft, low alternating tone while root beer is being poured. Like the
    /// menu music, this advances without blocking the game loop.
    pub fn play_pour_sound(&mut self) {
        if !self.tone_playing {
            self.note_active = false;
            self.pour_pitch_high = false;
            self.pour_pitch_started = Instant::now();
            self.play_tone(176, 28);
            return;
        }

        if self.pour_pitch_started.elapsed() >= Duration::from_millis(90) {
            self.pour_pitch_high = !self.pour_pitch_high;
            self.pour_pitch_started = Instant::now();
            let frequency = if self.pour_pitch_high { 220 } else { 176 };
            self.play_tone(frequency, 28);
        }
    }

    /// Starts a short scoring jingle. `advance_score_effect` must be called
    /// every frame until it returns false.
    pub fn play_score_effect(&mut self, result: ScoreResult) {
        if self.tone_playing {
            self.mute();
        }

        self.note_active = false;
        self.effect_notes = match result {
            // Bright rising sound for a target-level pour (+$10).
            ScoreResult::Perfect => [(523, 75), (659, 75), (784, 130)],
            // A small upward chirp for a partial pour (+$1).
            ScoreResult::SmallTip => [(392, 90), (523, 120), (0, 0)],
            // A low falling sound for an empty glass (+$0).
            ScoreResult::Miss => [(175, 100), (131, 150), (0, 0)],
        };
        self.effect_len = match result {
            ScoreResult::Perfect => 3,
            ScoreResult::SmallTip | ScoreResult::Miss => 2,
        };
        self.effect_index = 0;
        self.effect_started = Instant::now();
        self.effect_active = true;
        self.play_tone(self.effect_notes[0].0, 45);
    }

    /// Keeps a scoring jingle moving without blocking input or rendering.
    /// Returns true while the jingle owns the buzzer.
    pub fn advance_score_effect(&mut self) -> bool {
        if !self.effect_active {
            return false;
        }

        let (_, duration_ms) = self.effect_notes[self.effect_index];
        if self.effect_started.elapsed() < Duration::from_millis(duration_ms) {
            return true;
        }

        self.effect_index += 1;
        if self.effect_index >= self.effect_len {
            self.mute();
            self.effect_active = false;
            return false;
        }

        self.play_tone(self.effect_notes[self.effect_index].0, 45);
        self.effect_started = Instant::now();
        true
    }

    fn play_tone(&mut self, frequency_hz: u32, duty_pct: u8) {
        let mut hstimer0 = self.ledc.timer::<HighSpeed>(timer::Number::Timer0);
        hstimer0
            .configure(timer::config::Config {
                duty: timer::config::Duty::Duty10Bit,
                clock_source: timer::HSClockSource::APBClk,
                frequency: Rate::from_hz(frequency_hz),
            })
            .unwrap();
        let mut channel0 = self
            .ledc
            .channel::<HighSpeed>(channel::Number::Channel0, self.buzzer.reborrow());
        channel0
            .configure(channel::config::Config {
                timer: &hstimer0,
                duty_pct,
                drive_mode: DriveMode::PushPull,
            })
            .unwrap();
        self.tone_playing = true;
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
