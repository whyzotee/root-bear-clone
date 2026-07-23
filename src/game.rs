use embedded_graphics::{
    Drawable,
    geometry::Point,
    mono_font::{
        MonoTextStyleBuilder,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::BinaryColor,
    prelude::Primitive,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use esp_hal::{
    Async,
    i2c::master::I2c,
    time::{Duration, Instant},
};
use ssd1306::{
    Ssd1306, mode::BufferedGraphicsMode, prelude::I2CInterface, size::DisplaySize128x64,
};

use crate::button::Button;

const MINIMUM_TIP_LEVEL: u8 = 4;
const MAX_BEER_LEVEL: u8 = 27;

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

#[derive(Clone, Copy)]
pub enum ScoreResult {
    Perfect,
    SmallTip,
    Miss,
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
    menu_prompt_visible: bool,
    menu_blink_started: Instant,
    time_remaining: u8,
    game_timer_started: Instant,
    target_dash_offset: u8,
    target_line_started: Instant,
    target_beer_level: u8,
    is_pouring: bool,
    beer_level: u8,
    beer_fill_started: Instant,
    glass_shake_started: Instant,
    glass_shake_phase: u8,
    awaiting_pour_release: bool,
    customer: u8,
    pending_score_result: Option<ScoreResult>,
}

impl Game {
    pub fn new() -> Self {
        Self {
            score: 0,
            mode: GameMode::Classic,
            state: GameState::Menu,
            menu_prompt_visible: true,
            menu_blink_started: Instant::now(),
            time_remaining: 30,
            game_timer_started: Instant::now(),
            target_dash_offset: 0,
            target_line_started: Instant::now(),
            target_beer_level: 17,
            is_pouring: false,
            beer_level: 0,
            beer_fill_started: Instant::now(),
            glass_shake_started: Instant::now(),
            glass_shake_phase: 0,
            awaiting_pour_release: false,
            customer: 0,
            pending_score_result: None,
        }
    }

    pub fn show_game_menu(&mut self, display: &mut DisplayType) {
        self.draw_menu(display);
    }

    pub fn is_menu(&self) -> bool {
        matches!(self.state, GameState::Menu)
    }

    pub fn is_pouring(&self) -> bool {
        matches!(self.state, GameState::InGame) && self.is_pouring
    }

    pub fn take_score_result(&mut self) -> Option<ScoreResult> {
        self.pending_score_result.take()
    }

    /// Handles the menu button and redraws only when an animation value changes.
    pub fn process_frame(&mut self, display: &mut DisplayType, button: &mut Button) {
        match self.state {
            GameState::Menu => self.process_menu_frame(display, button),
            GameState::InGame => self.process_game_frame(display, button),
            GameState::GameOver => self.process_game_over_frame(display, button),
            GameState::Wom => {}
        }
    }

    fn process_menu_frame(&mut self, display: &mut DisplayType, button: &mut Button) {
        if button.just_preesed() {
            self.state = GameState::InGame;
            self.time_remaining = 30;
            self.score = 0;
            self.customer = 0;
            self.game_timer_started = Instant::now();
            self.target_dash_offset = 0;
            self.target_line_started = Instant::now();
            self.target_beer_level = 17;
            self.is_pouring = false;
            self.beer_level = 0;
            self.glass_shake_phase = 0;
            self.awaiting_pour_release = true;
            self.draw_game(display);
            return;
        }

        if self.menu_blink_started.elapsed() >= Duration::from_millis(500) {
            self.menu_prompt_visible = !self.menu_prompt_visible;
            self.menu_blink_started = Instant::now();
            self.draw_menu(display);
        }
    }

    fn process_game_frame(&mut self, display: &mut DisplayType, button: &mut Button) {
        let mut needs_redraw = false;

        // Shift the dashed target left to make the target level feel alive.
        if self.target_line_started.elapsed() >= Duration::from_millis(120) {
            self.target_dash_offset = (self.target_dash_offset + 1) % 4;
            self.target_line_started = Instant::now();
            needs_redraw = true;
        }

        if self.time_remaining > 0 && self.game_timer_started.elapsed() >= Duration::from_secs(1) {
            self.time_remaining -= 1;
            self.game_timer_started = Instant::now();
            if self.time_remaining == 0 {
                self.state = GameState::GameOver;
                self.draw_game_over(display);
                return;
            }
            needs_redraw = true;
        }

        // The press that entered the game must be released before it can pour.
        if self.awaiting_pour_release {
            if !button.is_held() {
                self.awaiting_pour_release = false;
            }
        } else if button.is_held() {
            if !self.is_pouring {
                self.is_pouring = true;
                self.beer_fill_started = Instant::now();
                self.glass_shake_started = Instant::now();
                self.glass_shake_phase = 0;
                needs_redraw = true;
            } else if self.beer_fill_started.elapsed() >= Duration::from_millis(80) {
                if self.beer_level < MAX_BEER_LEVEL {
                    // The target earns the best tip, but the player can pour
                    // past it until the glass is full.
                    self.beer_level += 1;
                } else {
                    // Holding past a full glass spills the drink: no tip, a
                    // new customer, and a release is required before retrying.
                    self.serve_customer(true);
                    self.awaiting_pour_release = true;
                }
                self.beer_fill_started = Instant::now();
                needs_redraw = true;
            }

            if self.glass_shake_started.elapsed() >= Duration::from_millis(55) {
                self.glass_shake_phase = (self.glass_shake_phase + 1) % 3;
                self.glass_shake_started = Instant::now();
                needs_redraw = true;
            }
        } else if self.is_pouring {
            self.serve_customer(false);
            needs_redraw = true;
        }

        if needs_redraw {
            self.draw_game(display);
        }
    }

    fn serve_customer(&mut self, overflowed: bool) {
        let result = if overflowed || self.beer_level < MINIMUM_TIP_LEVEL {
            ScoreResult::Miss
        } else if self.beer_level == self.target_beer_level {
            self.score += 10;
            ScoreResult::Perfect
        } else {
            self.score += 1;
            ScoreResult::SmallTip
        };
        self.pending_score_result = Some(result);

        self.customer = self.customer.wrapping_add(1);
        self.target_beer_level = 14 + self.customer % 5;
        self.beer_level = 0;
        self.is_pouring = false;
        self.glass_shake_phase = 0;
        self.target_dash_offset = 0;
    }

    fn process_game_over_frame(&mut self, display: &mut DisplayType, button: &mut Button) {
        if button.just_preesed() {
            self.state = GameState::Menu;
            self.menu_prompt_visible = true;
            self.menu_blink_started = Instant::now();
            self.draw_menu(display);
        }
    }

    fn draw_menu(&self, display: &mut DisplayType) {
        display.clear_buffer();

        let title_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(BinaryColor::On)
            .build();
        let body_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        // A small root-beer mug icon: foam, glass and a friendly bear face.
        Rectangle::new(
            Point::new(5, 7),
            embedded_graphics::geometry::Size::new(27, 35),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .unwrap();
        Circle::new(Point::new(8, 4), 8)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(21, 4), 8)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(10, 16), 4)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(23, 16), 4)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Line::new(Point::new(13, 29), Point::new(24, 29))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();

        Text::with_baseline("ROOT", Point::new(40, 4), title_style, Baseline::Top)
            .draw(display)
            .unwrap();
        Text::with_baseline("BEAR", Point::new(40, 24), title_style, Baseline::Top)
            .draw(display)
            .unwrap();

        Line::new(Point::new(4, 47), Point::new(123, 47))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();

        if self.menu_prompt_visible {
            Text::with_baseline(
                "PRESS TO START",
                Point::new(19, 53),
                body_style,
                Baseline::Top,
            )
            .draw(display)
            .unwrap();
        }

        display.flush().unwrap();
    }

    fn draw_game_over(&self, display: &mut DisplayType) {
        display.clear_buffer();
        let title_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(BinaryColor::On)
            .build();
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        Text::with_baseline("TIME'S UP", Point::new(19, 12), title_style, Baseline::Top)
            .draw(display)
            .unwrap();
        Text::with_baseline("TIP", Point::new(38, 37), text_style, Baseline::Top)
            .draw(display)
            .unwrap();
        let score = (self.score % 100) as u8;
        let score_text = [b'$', b'0' + score / 10, b'0' + score % 10];
        Text::with_baseline(
            core::str::from_utf8(&score_text).unwrap(),
            Point::new(62, 37),
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();
        Text::with_baseline(
            "PRESS FOR MENU",
            Point::new(19, 53),
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();
        display.flush().unwrap();
    }

    fn draw_game(&self, display: &mut DisplayType) {
        display.clear_buffer();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        // Top status bar: the countdown timer remains visible throughout a pour.
        Text::with_baseline("TIME", Point::new(84, 2), text_style, Baseline::Top)
            .draw(display)
            .unwrap();
        let timer = [
            b'0' + self.time_remaining / 10,
            b'0' + self.time_remaining % 10,
        ];
        Text::with_baseline(
            core::str::from_utf8(&timer).unwrap(),
            Point::new(108, 2),
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();
        Line::new(Point::new(2, 13), Point::new(125, 13))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();

        self.draw_bear(display);
        self.draw_bear_emote(display);
        self.draw_glass(display);

        // Bottom-left money jar and the current tip total.
        Circle::new(Point::new(5, 48), 13)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Rectangle::new(
            Point::new(8, 45),
            embedded_graphics::geometry::Size::new(7, 4),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)
        .unwrap();
        Line::new(Point::new(7, 60), Point::new(16, 60))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Text::with_baseline("$", Point::new(23, 52), text_style, Baseline::Top)
            .draw(display)
            .unwrap();
        let money = (self.score % 100) as u8;
        let money_text = [b'0' + money / 10, b'0' + money % 10];
        Text::with_baseline(
            core::str::from_utf8(&money_text).unwrap(),
            Point::new(30, 52),
            text_style,
            Baseline::Top,
        )
        .draw(display)
        .unwrap();

        display.flush().unwrap();
    }

    fn draw_bear(&self, display: &mut DisplayType) {
        // Customer on the left: ears, head, eyes, snout and body.
        Circle::new(Point::new(16, 17), 12)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(33, 17), 12)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(17, 22), 27)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(24, 31), 3)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(36, 31), 3)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(26, 37), 10)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(30, 39), 3)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();
        Rectangle::new(
            Point::new(16, 49),
            embedded_graphics::geometry::Size::new(30, 13),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)
        .unwrap();
    }

    fn draw_bear_emote(&self, display: &mut DisplayType) {
        // The open space between the customer and glass becomes a compact
        // speech bubble, so the bear reacts without obscuring the pour guide.
        let emote = if !self.is_pouring {
            "..."
        } else if self.beer_level >= self.target_beer_level {
            "^_^"
        } else {
            "o_o"
        };
        let bubble_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        Rectangle::new(
            Point::new(51, 16),
            embedded_graphics::geometry::Size::new(29, 14),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .unwrap();
        Line::new(Point::new(53, 29), Point::new(47, 33))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Line::new(Point::new(47, 33), Point::new(56, 29))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Text::with_baseline(emote, Point::new(57, 18), bubble_style, Baseline::Top)
            .draw(display)
            .unwrap();
    }

    fn draw_glass(&self, display: &mut DisplayType) {
        const GLASS_LEFT: i32 = 91;
        const GLASS_TOP: i32 = 23;
        let shake_x = if self.is_pouring {
            match self.glass_shake_phase {
                0 => -1,
                1 => 1,
                _ => 0,
            }
        } else {
            0
        };
        let target_y = 55 - self.target_beer_level as i32;

        // The glass is deliberately drawn before its contents and target marker.
        Rectangle::new(
            Point::new(GLASS_LEFT + shake_x, GLASS_TOP),
            embedded_graphics::geometry::Size::new(25, 32),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)
        .unwrap();
        Line::new(Point::new(94 + shake_x, 28), Point::new(113 + shake_x, 28))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Circle::new(Point::new(115 + shake_x, 30), 12)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(display)
            .unwrap();
        Line::new(Point::new(93 + shake_x, 58), Point::new(115 + shake_x, 58))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();

        if self.beer_level > 0 {
            let beer_top = 55 - self.beer_level as i32;
            Rectangle::new(
                Point::new(93 + shake_x, beer_top),
                embedded_graphics::geometry::Size::new(21, self.beer_level as u32),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .unwrap();

            // Tiny negative-space pixels make a distinct, blinking foam layer.
            // They move one pixel every animation tick without blending into beer.
            let foam_shift = (self.target_dash_offset % 2) as i32;
            for bubble in 0..6 {
                let x = 94 + bubble * 3 + foam_shift + shake_x;
                Line::new(Point::new(x, beer_top + 1), Point::new(x + 1, beer_top + 1))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 1))
                    .draw(display)
                    .unwrap();
            }
        }

        // Animated dashes travel from right to left across the target fill level.
        let dash_color = if self.beer_level >= self.target_beer_level {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };
        for dash in 0..4 {
            // At every animation phase, the full dash remains within x=93..113.
            let right = 98 + dash * 5 - self.target_dash_offset as i32 + shake_x;
            Line::new(Point::new(right - 2, target_y), Point::new(right, target_y))
                .into_styled(PrimitiveStyle::with_stroke(dash_color, 1))
                .draw(display)
                .unwrap();
        }

        if self.is_pouring {
            self.draw_target_finger(display, target_y);
        }
    }

    fn draw_target_finger(&self, display: &mut DisplayType, target_y: i32) {
        // A small hand with an extended index finger points exactly at the mark.
        Circle::new(Point::new(73, target_y - 4), 9)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
        Line::new(Point::new(79, target_y), Point::new(90, target_y))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
            .draw(display)
            .unwrap();
        Line::new(Point::new(79, target_y + 3), Point::new(85, target_y + 3))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .unwrap();
    }
}
