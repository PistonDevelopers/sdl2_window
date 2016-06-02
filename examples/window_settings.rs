
extern crate sdl2_window;
extern crate window;

use sdl2_window::Sdl2Window;
use window::WindowSettings;

fn main() {
    let _ = Sdl2Window::new(
        &WindowSettings::new("SDL Window", (640, 480))
            .fullscreen(false)
            .vsync(true) // etc
    ).unwrap();
}
