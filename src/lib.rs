#![crate_name = "sdl2_game_window"]
#![deny(missing_doc)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate piston;
extern crate shader_version;
extern crate gl;
extern crate gfx;
extern crate device;

pub use window_sdl2::WindowSDL2;

mod window_sdl2;

