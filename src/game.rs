use embedded_graphics::{
    Drawable,
    geometry::Point,
    mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10},
    pixelcolor::BinaryColor,
    text::{Baseline, Text},
};
use esp_hal::{Async, i2c::master::I2c};
use ssd1306::{
    Ssd1306, mode::BufferedGraphicsMode, prelude::I2CInterface, size::DisplaySize128x64,
};

pub type DisplayType<'a> = Ssd1306<
    I2CInterface<I2c<'a, Async>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

pub enum GameState {
    Menu,
    InGame,
    GameOver,
    Wom,
}

enum GameMode {
    Scores,
    Classic,
    Endless,
    Flawless,
}

pub struct Game {
    score: u64,
    mode: GameMode,
    state: GameState,
}

impl Game {
    pub fn new() -> Self {
        Self {
            score: 0,
            mode: GameMode::Classic,
            state: GameState::Menu,
        }
    }

    pub fn show_game_menu(&mut self, display: &mut DisplayType) {
        self.clean_screen(display);

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        Text::with_baseline(
            "Tap Button to start!",
            Point::zero(),
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        display.flush().unwrap();
    }

    pub fn process_frame(&mut self, display: &mut DisplayType) {}

    fn clean_screen(&self, display: &mut DisplayType) {
        display.clear_buffer();
        display.flush().unwrap();
    }
}
