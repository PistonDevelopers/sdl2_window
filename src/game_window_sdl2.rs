//! A window implemented by SDL2 back-end.

// External crates.
use std::mem::transmute;
use gl;
use gfx;
use device;
use sdl2;
use piston::{
    GameWindow,
    GameWindowSettings,
    game_window,
    keyboard,
    mouse,
};
use shader_version::opengl::OpenGL;

/// A widow implemented by SDL2 back-end.
pub struct GameWindowSDL2 {
    /// SDL window handle
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    settings: GameWindowSettings,
    should_close: bool,
    last_pressed_key: Option<sdl2::keycode::KeyCode>,
}

impl GameWindowSDL2 {
    /// Creates a new game window for SDL2.
    pub fn new(opengl: OpenGL, settings: GameWindowSettings) -> GameWindowSDL2 {
        sdl2::init(sdl2::InitEverything);
        let (major, minor) = opengl.get_major_minor();
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMajorVersion, major);
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMinorVersion, minor);
        sdl2::video::gl_set_attribute(
            sdl2::video::GLContextProfileMask, 
            sdl2::video::ll::SDL_GL_CONTEXT_PROFILE_CORE as int
        );

        let window = sdl2::video::Window::new(
            settings.title.as_slice(),
            sdl2::video::PosCentered,
            sdl2::video::PosCentered,
            settings.size[0] as int,
            settings.size[1] as int,
            sdl2::video::OpenGL | sdl2::video::Resizable
        ).unwrap();
        if settings.fullscreen {
            window.set_fullscreen(sdl2::video::FTTrue);
        }

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers
        gl::load_with(|s| unsafe {
            transmute(sdl2::video::gl_get_proc_address(s))
        });

        GameWindowSDL2 {
            settings: settings,
            should_close: false,
            last_pressed_key: None,
            window: window,
            context: context,
        }
    }

    /// Creates a gfx devince and front end.
    pub fn gfx(&self) -> (device::GlDevice, gfx::Frame) {
        let device = device::GlDevice::new(|s| unsafe {
            transmute(sdl2::video::gl_get_proc_address(s))
        });
        let (w, h) = self.get_size();
        let frame = gfx::Frame::new(w as u16, h as u16);
        (device, frame)
    }
}

impl GameWindow for GameWindowSDL2 {
    fn get_settings<'a>(&'a self) -> &'a GameWindowSettings {
        &self.settings
    }

    fn should_close(&self) -> bool {
        self.should_close
    }

    fn get_draw_size(&self) -> (u32, u32) {
        let (w, h) = self.window.get_drawable_size();
        (w as u32, h as u32)
    }

    fn close(&mut self) {
        self.should_close = true;
    }

    fn swap_buffers(&self) {
        self.window.gl_swap_window();
    }

    fn capture_cursor(&mut self, enabled: bool) {
        sdl2::mouse::set_relative_mouse_mode(enabled)
    }

    fn poll_event(&mut self) -> game_window::Event {
        match sdl2::event::poll_event() {
            sdl2::event::QuitEvent(_) => { self.should_close = true; },
            sdl2::event::KeyDownEvent(_, _, key, _, _) => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                match self.last_pressed_key {
                    Some(x) if x == key => return self.poll_event(),
                    _ => {},
                };
                self.last_pressed_key = Some(key);

                if self.settings.exit_on_esc
                && key == sdl2::keycode::EscapeKey {
                    self.should_close = true;
                } else {
                    return game_window::KeyPressed(sdl2_map_key(key));
                }
            },
            sdl2::event::KeyUpEvent(_, _, key, _, _) => {
                // Reset the last pressed key.
                self.last_pressed_key = match self.last_pressed_key {
                    Some(x) if x == key => None,
                    x => x,
                };

                return game_window::KeyReleased(sdl2_map_key(key));
            },
            sdl2::event::MouseButtonDownEvent(_, _, _, button, _, _) => {
                return game_window::MouseButtonPressed(sdl2_map_mouse(button));
            },
            sdl2::event::MouseButtonUpEvent(_, _, _, button, _, _) => {
                return game_window::MouseButtonReleased(sdl2_map_mouse(button));
            },
            sdl2::event::MouseMotionEvent(_, _, _, _, x, y, dx, dy) => {
                return game_window::MouseMoved(
                    x as f64,
                    y as f64,
                    Some((dx as f64, dy as f64))
                );
            },
            sdl2::event::MouseWheelEvent(_, _, _, x, y) => {
                return game_window::MouseScrolled(x as f64, y as f64);
            },
            _ => {},
        }
        game_window::NoEvent
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

