#![deny(missing_doc)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate event;
extern crate shader_version;
extern crate input;
extern crate gl;

pub use window_sdl2::WindowSDL2;

mod window_sdl2;

