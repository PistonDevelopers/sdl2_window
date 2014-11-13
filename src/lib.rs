#![deny(missing_docs)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate shader_version;
extern crate input;
extern crate gl;
extern crate current;

// External crates.
use std::mem::transmute;
use window::{
    Window,
    WindowSettings,
    ShouldClose, Size, PollEvent, SwapBuffers,
    CaptureCursor, DrawSize, Title, ExitOnEsc
};
use input::{ keyboard, mouse, InputEvent };
use shader_version::opengl::OpenGL;
use current::{ Get, Modifier, Set };

/// A widow implemented by SDL2 back-end.
pub struct Sdl2Window {
    /// SDL window handle
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    should_close: bool,
    last_pressed_key: Option<sdl2::keycode::KeyCode>,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2.
    pub fn new(opengl: OpenGL, settings: WindowSettings) -> Sdl2Window {
        sdl2::init(sdl2::INIT_EVERYTHING);
        let (major, minor) = opengl.get_major_minor();
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMajorVersion, major);
        sdl2::video::gl_set_attribute(sdl2::video::GLContextMinorVersion, minor);
        sdl2::video::gl_set_attribute(
            sdl2::video::GLContextProfileMask,
            sdl2::video::ll::SDL_GL_CONTEXT_PROFILE_CORE as int
        );
        if settings.samples != 0 {
            sdl2::video::gl_set_attribute(sdl2::video::GLMultiSampleBuffers, 1);
            sdl2::video::gl_set_attribute(
                sdl2::video::GLMultiSampleSamples,
                settings.samples as int
            );
        }

        let window = sdl2::video::Window::new(
            settings.title.as_slice(),
            sdl2::video::PosCentered,
            sdl2::video::PosCentered,
            settings.size[0] as int,
            settings.size[1] as int,
            sdl2::video::OPENGL| sdl2::video::RESIZABLE
        ).unwrap();
        if settings.fullscreen {
            window.set_fullscreen(sdl2::video::FTTrue);
        }

        // Send text input events.
        sdl2::keyboard::start_text_input();

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers
        gl::load_with(|s| unsafe {
            transmute(sdl2::video::gl_get_proc_address(s))
        });

        Sdl2Window {
            exit_on_esc: settings.exit_on_esc,
            should_close: false,
            last_pressed_key: None,
            window: window,
            context: context,
            mouse_relative: None,
        }
    }
}

impl Drop for Sdl2Window {
    fn drop(&mut self) {
        self.set_mut(CaptureCursor(false));
    }
}

impl Get<ShouldClose> for Sdl2Window {
    fn get(&self) -> ShouldClose {
        ShouldClose(self.should_close)
    }
}

impl Get<Size> for Sdl2Window {
    fn get(&self) -> Size {
        let (w, h) = self.window.get_size();
        Size([w as u32, h as u32])
    }
}

impl SwapBuffers for Sdl2Window {
    fn swap_buffers(&mut self) {
        self.window.gl_swap_window();
    }
}

impl PollEvent<InputEvent> for Sdl2Window {
    fn poll_event(&mut self) -> Option<InputEvent> {
        match self.mouse_relative {
            Some((x, y)) => {
                self.mouse_relative = None;
                return Some(input::Move(input::MouseRelative(x, y)));
            }
            None => {}
        }
        match sdl2::event::poll_event() {
            sdl2::event::QuitEvent(_) => { self.should_close = true; }
            sdl2::event::TextInputEvent(_, _, text) => { return Some(input::Text(text)); }
            sdl2::event::KeyDownEvent(_, _, key, _, _) => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                match self.last_pressed_key {
                    Some(x) if x == key => return self.poll_event(),
                    _ => {}
                };
                self.last_pressed_key = Some(key);

                if self.exit_on_esc
                && key == sdl2::keycode::EscapeKey {
                    self.should_close = true;
                } else {
                    return Some(input::Press(input::Keyboard(sdl2_map_key(key))));
                }
            }
            sdl2::event::KeyUpEvent(_, _, key, _, _) => {
                // Reset the last pressed key.
                self.last_pressed_key = match self.last_pressed_key {
                    Some(x) if x == key => None,
                    x => x,
                };

                return Some(input::Release(input::Keyboard(sdl2_map_key(key))));
            }
            sdl2::event::MouseButtonDownEvent(_, _, _, button, _, _) => {
                return Some(input::Press(input::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::MouseButtonUpEvent(_, _, _, button, _, _) => {
                return Some(input::Release(input::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::MouseMotionEvent(_, _, _, _, x, y, dx, dy) => {
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64));
                return Some(input::Move(input::MouseCursor(x as f64, y as f64)));
            },
            sdl2::event::MouseWheelEvent(_, _, _, x, y) => {
                return Some(input::Move(input::MouseScroll(x as f64, y as f64)));
            }
            sdl2::event::WindowEvent(_, _, sdl2::event::ResizedWindowEventId, w, h) => {
                return Some(input::Resize(w as u32, h as u32));
            }
            sdl2::event::WindowEvent(_, _, sdl2::event::FocusGainedWindowEventId, _, _) => {
                return Some(input::Focus(true));
            }
            sdl2::event::WindowEvent(_, _, sdl2::event::FocusLostWindowEventId, _, _) => {
                return Some(input::Focus(false));
            }
            _ => {}
        }
        None
    }
}

impl Modifier<Sdl2Window> for CaptureCursor {
    fn modify(self, _window: &mut Sdl2Window) {
        let CaptureCursor(enabled) = self;
        sdl2::mouse::set_relative_mouse_mode(enabled)
    }
}

impl Modifier<Sdl2Window> for ShouldClose {
    fn modify(self, window: &mut Sdl2Window) {
        let ShouldClose(val) = self;
        window.should_close = val;
    }
}

impl Get<DrawSize> for Sdl2Window {
    fn get(&self) -> DrawSize {
        let (w, h) = self.window.get_drawable_size();
        DrawSize([w as u32, h as u32])
    }
}

impl Get<Title> for Sdl2Window {
    fn get(&self) -> Title {
        Title(self.window.get_title())
    }
}

impl Modifier<Sdl2Window> for Title {
    fn modify(self, window: &mut Sdl2Window) {
        let Title(val) = self;
        window.window.set_title(val.as_slice());
    }
}

impl Get<ExitOnEsc> for Sdl2Window {
    fn get(&self) -> ExitOnEsc {
        ExitOnEsc(self.exit_on_esc)
    }
}

impl Modifier<Sdl2Window> for ExitOnEsc {
    fn modify(self, window: &mut Sdl2Window) {
        let ExitOnEsc(val) = self;
        window.exit_on_esc = val;
    }
}

impl Window for Sdl2Window {}

/// Maps a SDL2 key to piston-input key.
pub fn sdl2_map_key(keycode: sdl2::keycode::KeyCode) -> keyboard::Key {
    use std::num::FromPrimitive;
    FromPrimitive::from_u64(keycode as u64).unwrap()
}

/// Maps a SDL2 mouse button to piston-input button.
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
