#![crate_name = "sdl2_game_window"]
#![deny(missing_doc)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate piston;
extern crate gl;
extern crate gfx;
extern crate device;

pub use game_window_sdl2::GameWindowSDL2;

mod game_window_sdl2;

