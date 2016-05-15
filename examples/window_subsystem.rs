
extern crate sdl2;
extern crate sdl2_window;
extern crate window;

use sdl2_window::Sdl2Window;
use window::WindowSettings;

fn main() {
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();
    
    let _ = Sdl2Window::with_subsystem(
        video_subsystem,
        WindowSettings::new("SDL Window", (640, 480))
            .fullscreen(false)
            .vsync(true) // etc
    ).unwrap();
}