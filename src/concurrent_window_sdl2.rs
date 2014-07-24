//! RenderWindow and ConcurrentWindow implemented by SDL2 back-end.

// External crates.
use std;
use sdl2;
use piston::{
    GameWindow,
    RenderWindow,
    GameWindowSettings,
    event,
};
use gl;

// Local Crate.
use game_window_sdl2::sdl2_map_key;
use game_window_sdl2::sdl2_map_mouse;

/// A widow implemented by SDL2 back-end.
pub struct RenderWindowSDL2 {
    window: sdl2::video::Window,
    // Allow dead code because this keeps track of the OpenGL context.
    // Will be released on drop.
    #[allow(dead_code)]
    context: sdl2::video::GLContext,
}

impl RenderWindow for RenderWindowSDL2 {
    fn swap_buffers(&self) {
        self.window.gl_swap_window();
    }

    fn capture_cursor(&mut self, enabled: bool) {
        sdl2::mouse::set_relative_mouse_mode(enabled)
    }
}

/// A window implemented by SDL2 back-end.
pub struct ConcurrentWindowSDL2 {
    settings: GameWindowSettings,
    should_close: bool,
    last_pressed_key: Option<sdl2::keycode::KeyCode>,
}

impl GameWindow for ConcurrentWindowSDL2 {
    fn get_settings<'a>(&'a self) -> &'a GameWindowSettings {
        &self.settings
    }

    fn should_close(&self) -> bool {
        self.should_close
    }

    fn poll_event(&mut self) -> event::Event {
        match sdl2::event::poll_event() {
            sdl2::event::QuitEvent(_) => { self.should_close = true; },
            sdl2::event::KeyDownEvent(_, _, key, _, _) => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                match self.last_pressed_key {
                    Some(x) if x == key => return event::NoEvent,
                    _ => {},
                };
                self.last_pressed_key = Some(key);

                if self.settings.exit_on_esc
                && key == sdl2::keycode::EscapeKey {
                    self.should_close = true;
                } else {
                    return event::KeyPressed(sdl2_map_key(key));
                }
            },
            sdl2::event::KeyUpEvent(_, _, key, _, _) => {
                // Reset the last pressed key.
                self.last_pressed_key = match self.last_pressed_key {
                    Some(x) if x == key => None,
                    x => x,
                };

                return event::KeyReleased(sdl2_map_key(key));
            },
            sdl2::event::MouseButtonDownEvent(_, _, _, button, _, _) => {
                return event::MouseButtonPressed(sdl2_map_mouse(button));
            },
            sdl2::event::MouseButtonUpEvent(_, _, _, button, _, _) => {
                return event::MouseButtonReleased(sdl2_map_mouse(button));
            },
            sdl2::event::MouseMotionEvent(_, _, _, _, x, y, dx, dy) => {
                return event::MouseMoved(
                    x as f64,
                    y as f64,
                    Some((dx as f64, dy as f64))
                );
            },
            sdl2::event::MouseWheelEvent(_, _, _, x, y) => {
                return event::MouseScrolled(x as f64, y as f64);
            },
            _ => {},
        }
        event::NoEvent
    }
}

impl ConcurrentWindowSDL2 {
    /// Creates a new game window for SDL2.
    pub fn new(settings: GameWindowSettings) -> (ConcurrentWindowSDL2, RenderWindowSDL2) {
        sdl2::init(sdl2::InitEverything);
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMajorVersion, 3);
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMinorVersion, 3);
        sdl2::video::gl_set_attribute(sdl2::video::GLContextProfileMask, sdl2::video::ll::SDL_GL_CONTEXT_PROFILE_CORE as int);

        let window = sdl2::video::Window::new(
            settings.title.as_slice(),
            sdl2::video::PosCentered,
            sdl2::video::PosCentered,
            settings.size[0] as int,
            settings.size[1] as int,
            sdl2::video::OpenGL
        ).unwrap();
        if settings.fullscreen {
            window.set_fullscreen(sdl2::video::FTTrue);
        }

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers
        gl::load_with(|s| unsafe {
            std::mem::transmute(sdl2::video::gl_get_proc_address(s))
        });

        return (
            ConcurrentWindowSDL2 {
                settings: settings,
                should_close: false,
                last_pressed_key: None,
            },
            RenderWindowSDL2 {
                window: window,
                context: context,
            }
        );
                
    }
}
