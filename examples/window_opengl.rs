extern crate sdl2_window;
extern crate shader_version;
extern crate window;

use sdl2_window::Sdl2Window;
use shader_version::OpenGL;
use window::WindowSettings;

fn main() {
    let _ = Sdl2Window::new(
        &WindowSettings::new("SDL Window", (640, 480))
            .fullscreen(false)
            .vsync(true)
            .graphics_api(OpenGL::V2_1), // etc
    )
    .unwrap();
}
