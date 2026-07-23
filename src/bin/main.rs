#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Pin;
use esp_hal::i2c::master::{Config as ConfigI2C, I2c};
use esp_hal::main;
use esp_hal::time::Rate;
use root_bear_clone::button::Button;
use root_bear_clone::game::Game;
use root_bear_clone::sound::Sound;
use ssd1306::mode::DisplayConfig;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306};

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32 -o log -o esp-backtrace -o esp32-wroom-32 -o ci -o vscode

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // The following pins are used to bootstrap the chip. They are available
    // for use, but check the datasheet of the module for more information on them.
    // - GPIO0
    // - GPIO2
    // - GPIO5
    // - GPIO12
    // - GPIO15
    // These GPIO pins are in use by some feature of the module and should not be used.
    let _ = peripherals.GPIO6;
    let _ = peripherals.GPIO7;
    let _ = peripherals.GPIO8;
    let _ = peripherals.GPIO9;
    let _ = peripherals.GPIO10;
    let _ = peripherals.GPIO11;
    let _ = peripherals.GPIO16;
    let _ = peripherals.GPIO20;

    let mut button = Button::new(peripherals.GPIO4.degrade());

    let i2c = I2c::new(
        peripherals.I2C0,
        ConfigI2C::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO21)
    .with_scl(peripherals.GPIO22);

    let i2c = i2c.into_async();

    let interface = I2CDisplayInterface::new(i2c);

    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    let mut game = Game::new();
    let mut song = Sound::new(peripherals.GPIO13.degrade(), peripherals.LEDC);

    game.show_game_menu(&mut display);

    loop {
        game.process_frame(&mut display, &mut button);
        if let Some(result) = game.take_score_result() {
            song.play_score_effect(result);
        }

        if song.advance_score_effect() {
            continue;
        } else if game.is_menu() {
            song.play_menu_music();
        } else if game.is_pouring() {
            song.play_pour_sound();
        } else {
            song.stop();
        }
    }
}
