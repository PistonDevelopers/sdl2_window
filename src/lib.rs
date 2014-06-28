#![crate_id = "sdl2_game_window"]
#![deny(missing_doc)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate piston;
extern crate gl;

pub use GameWindowSDL2 = game_window_sdl2::GameWindowSDL2;
pub use ConcurrentWindowSDL2 = concurrent_window_sdl2::ConcurrentWindowSDL2;
pub use RenderWindowSDL2 = concurrent_window_sdl2::RenderWindowSDL2;

mod game_window_sdl2;
mod concurrent_window_sdl2;

