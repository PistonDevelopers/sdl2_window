//! A window implemented by SDL2 back-end.

// External crates.
use sdl2;
use piston::{
    GameWindow,
    RenderWindow,
    GameWindowSettings,
    event,
    keyboard,
    mouse,
};

// Local Crate.

use concurrent_window_sdl2::{
    RenderWindowSDL2,
    ConcurrentWindowSDL2,
};


/// A widow implemented by SDL2 back-end.
pub struct GameWindowSDL2 {
    concurrent_window: ConcurrentWindowSDL2,
    render_window: RenderWindowSDL2,
}

impl GameWindowSDL2 {
    /// Creates a new game window for SDL2.
    pub fn new(settings: GameWindowSettings) -> GameWindowSDL2 {
        let (concurrent_window, render_window) = ConcurrentWindowSDL2::new( settings );
        GameWindowSDL2 {
            concurrent_window: concurrent_window,
            render_window: render_window,
        }
    }
}

impl GameWindow for GameWindowSDL2 {
    fn get_settings<'a>(&'a self) -> &'a GameWindowSettings {
        self.concurrent_window.get_settings()
    }

    fn should_close(&self) -> bool {
        self.concurrent_window.should_close()
    }

    fn swap_buffers(&self) {
        self.render_window.swap_buffers()
    }

    fn poll_event(&mut self) -> event::Event {
        self.concurrent_window.poll_event()
    }
}

pub fn sdl2_map_key(keycode: sdl2::keycode::KeyCode) -> keyboard::Key {
    use std::num::FromPrimitive;
    FromPrimitive::from_u64(keycode.code() as u64).unwrap()
}

pub fn sdl2_map_mouse(button: sdl2::mouse::Mouse) -> mouse::Button {
    match button {
        sdl2::mouse::LeftMouse => mouse::Left,
        sdl2::mouse::RightMouse => mouse::Right,
        sdl2::mouse::MiddleMouse => mouse::Middle,
        sdl2::mouse::X1Mouse => mouse::X1,
        sdl2::mouse::X2Mouse => mouse::X2,
        sdl2::mouse::UnknownMouse(_) => mouse::Unknown,
    }
}


