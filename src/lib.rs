#![deny(missing_docs)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate piston;
extern crate shader_version;
extern crate gl;
extern crate num;

// External crates.
use std::mem::transmute;
use piston::window::{
    OpenGLWindow,
    ProcAddress,
    Window,
    AdvancedWindow,
    WindowSettings,
    Size,
};
use piston::input::{ keyboard, Button, MouseButton, Input, Motion };

pub use shader_version::OpenGL;

/// A widow implemented by SDL2 back-end.
pub struct Sdl2Window {
    /// SDL window handle.
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    /// SDL context.
    pub sdl_context: sdl2::Sdl,
    should_close: bool,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2.
    pub fn new(opengl: OpenGL, settings: WindowSettings) -> Sdl2Window {
        let sdl_context = sdl2::init(sdl2::INIT_EVERYTHING).unwrap();

        // Not all drivers default to 32bit color, so explicitly set it to 32bit color.
        sdl2::video::gl_set_attribute(sdl2::video::GLAttr::GLRedSize, 8);
        sdl2::video::gl_set_attribute(sdl2::video::GLAttr::GLGreenSize, 8);
        sdl2::video::gl_set_attribute(sdl2::video::GLAttr::GLBlueSize, 8);
        sdl2::video::gl_set_attribute(sdl2::video::GLAttr::GLAlphaSize, 8);

        let (major, minor) = opengl.get_major_minor();
        sdl2::video::gl_set_attribute(
            sdl2::video::GLAttr::GLContextMajorVersion,
            major as i32
        );
        sdl2::video::gl_set_attribute(
            sdl2::video::GLAttr::GLContextMinorVersion,
            minor as i32
        );
        sdl2::video::gl_set_attribute(
            sdl2::video::GLAttr::GLContextProfileMask,
            sdl2::video::GLProfile::GLCoreProfile as i32
        );
        if settings.get_samples() != 0 {
            sdl2::video::gl_set_attribute(
                sdl2::video::GLAttr::GLMultiSampleBuffers,
                1
            );
            sdl2::video::gl_set_attribute(
                sdl2::video::GLAttr::GLMultiSampleSamples,
                settings.get_samples() as i32
            );
        }

        let window = sdl2::video::Window::new(
            &settings.get_title(),
            sdl2::video::WindowPos::PosCentered,
            sdl2::video::WindowPos::PosCentered,
            settings.get_size().width as i32,
            settings.get_size().height as i32,
            sdl2::video::OPENGL| sdl2::video::RESIZABLE
        );
        let window = match window {
            Ok(w) => w,
            Err(_) =>
                if settings.get_samples() != 0 {
                    // Retry without requiring anti-aliasing.
                    sdl2::video::gl_set_attribute(
                        sdl2::video::GLAttr::GLMultiSampleBuffers,
                        0
                            );
                    sdl2::video::gl_set_attribute(
                        sdl2::video::GLAttr::GLMultiSampleSamples,
                        0
                            );
                    sdl2::video::Window::new(
                        &settings.get_title(),
                        sdl2::video::WindowPos::PosCentered,
                        sdl2::video::WindowPos::PosCentered,
                        settings.get_size().width as i32,
                        settings.get_size().height as i32,
                        sdl2::video::OPENGL| sdl2::video::RESIZABLE
                            ).unwrap()
                } else {
                    window.unwrap() // Panic.
                }
        };
        if settings.get_fullscreen() {
            window.set_fullscreen(sdl2::video::FullscreenType::FTTrue);
        }

        // Send text input events.
        sdl2::keyboard::start_text_input();

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers.
        gl::load_with(|s| unsafe {
            transmute(sdl2::video::gl_get_proc_address(s))
        });

        Sdl2Window {
            exit_on_esc: settings.get_exit_on_esc(),
            should_close: false,
            window: window,
            context: context,
            sdl_context: sdl_context,
            mouse_relative: None,
        }
    }

    fn poll_event(&mut self) -> Option<Input> {
        // First check for a pending relative mouse move event.
        match self.mouse_relative {
            Some((x, y)) => {
                self.mouse_relative = None;
                return Some(Input::Move(Motion::MouseRelative(x, y)));
            }
            None => {}
        }

        // Even though we create a new EventPump each time we poll an event
        // this should not be a problem since it only contains phantom data
        // and therefore should actually not have any overhead.
        let event = match self.sdl_context.event_pump().poll_event() {
            Some( ev ) => ev,
            None => return None
        };
        match event {
            sdl2::event::Event::Quit{..} => {
                self.should_close = true;
            }
            sdl2::event::Event::TextInput { text, .. } => {
                return Some(Input::Text(text));
            }
            sdl2::event::Event::KeyDown { keycode: key, repeat, ..} => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                if repeat {
                    return self.poll_event()
                }

                if self.exit_on_esc
                && key == sdl2::keycode::KeyCode::Escape {
                    self.should_close = true;
                } else {
                    return Some(Input::Press(Button::Keyboard(sdl2_map_key(key))));
                }
            }
            sdl2::event::Event::KeyUp { keycode: key, repeat, .. } => {
                if repeat {
                    return self.poll_event()
                }
                return Some(Input::Release(Button::Keyboard(sdl2_map_key(key))));
            }
            sdl2::event::Event::MouseButtonDown { mouse_btn: button, .. } => {
                return Some(Input::Press(Button::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::Event::MouseButtonUp { mouse_btn: button, .. } => {
                return Some(Input::Release(Button::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::Event::MouseMotion { x, y, xrel: dx, yrel: dy, .. } => {
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64));
                return Some(Input::Move(Motion::MouseCursor(x as f64, y as f64)));
            },
            sdl2::event::Event::MouseWheel { x, y, .. } => {
                return Some(Input::Move(Motion::MouseScroll(x as f64, y as f64)));
            }
            sdl2::event::Event::Window {
                win_event_id: sdl2::event::WindowEventId::Resized, data1: w, data2: h, .. } => {
                return Some(Input::Resize(w as u32, h as u32));
            }
            sdl2::event::Event::Window { win_event_id: sdl2::event::WindowEventId::FocusGained, .. } => {
                return Some(Input::Focus(true));
            }
            sdl2::event::Event::Window { win_event_id: sdl2::event::WindowEventId::FocusLost, .. } => {
                return Some(Input::Focus(false));
            }
            _ => {}
        }
        None
    }
}

impl Drop for Sdl2Window {
    fn drop(&mut self) {
        self.set_capture_cursor(false);
    }
}

impl Window for Sdl2Window {
    type Event = Input;

    fn should_close(&self) -> bool { self.should_close }
    fn swap_buffers(&mut self) { self.window.gl_swap_window(); }
    fn size(&self) -> Size {
        let (w, h) = self.window.get_size();
        Size { width: w as u32, height: h as u32 }
    }
    fn poll_event(&mut self) -> Option<Input> { self.poll_event() }
}

impl AdvancedWindow for Sdl2Window {
    fn get_title(&self) -> String { self.window.get_title() }
    fn set_title(&mut self, value: String) { let _ = self.window.set_title(&value); }
    fn get_exit_on_esc(&self) -> bool { self.exit_on_esc }
    fn set_exit_on_esc(&mut self, value: bool) { self.exit_on_esc = value; }
    fn set_capture_cursor(&mut self, value: bool) {
        sdl2::mouse::set_relative_mouse_mode(value);
    }
    fn draw_size(&self) -> Size {
        let (w, h) = self.window.get_size();
        Size { width: w as u32, height: h as u32 }
    }
}

impl OpenGLWindow for Sdl2Window {
    fn get_proc_address(&mut self, proc_name: &str) -> ProcAddress {
        unsafe {
            transmute(sdl2::video::gl_get_proc_address(proc_name))
        }
    }

    fn is_current(&self) -> bool {
        unsafe {
            let this_context = self.context.raw();
            let current_context = sdl2::video::gl_get_current_context().unwrap().raw();

            this_context == current_context
        }
    }

    fn make_current(&mut self) {
        self.window.gl_make_current(&self.context);
    }
}

/// Maps a SDL2 key to piston-input key.
pub fn sdl2_map_key(keycode: sdl2::keycode::KeyCode) -> keyboard::Key {
    use num::FromPrimitive;
    FromPrimitive::from_u64(keycode as u64).unwrap()
}

/// Maps a SDL2 mouse button to piston-input button.
pub fn sdl2_map_mouse(button: sdl2::mouse::Mouse) -> MouseButton {
    match button {
        sdl2::mouse::Mouse::Left => MouseButton::Left,
        sdl2::mouse::Mouse::Right => MouseButton::Right,
        sdl2::mouse::Mouse::Middle => MouseButton::Middle,
        sdl2::mouse::Mouse::X1 => MouseButton::X1,
        sdl2::mouse::Mouse::X2 => MouseButton::X2,
        sdl2::mouse::Mouse::Unknown(_) => MouseButton::Unknown,
    }
}
