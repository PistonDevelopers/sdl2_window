#![deny(missing_docs)]
#![feature(core)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate shader_version;
extern crate input;
extern crate gl;
extern crate libc;
#[macro_use]
extern crate quack;

// External crates.
use std::mem::transmute;
use window::{
    WindowSettings,
    ShouldClose, Size, PollEvent, SwapBuffers,
    CaptureCursor, DrawSize, Title, ExitOnEsc
};
use input::{ keyboard, Button, MouseButton, Input, Motion };
use shader_version::OpenGL;
use quack::{ Associative, Set };

/// A widow implemented by SDL2 back-end.
pub struct Sdl2Window {
    /// SDL window handle
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    /// SDL context
    pub sdl_context: sdl2::Sdl,
    should_close: bool,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2.
    pub fn new(opengl: OpenGL, settings: WindowSettings) -> Sdl2Window {
        let sdl_context = sdl2::init(sdl2::INIT_EVERYTHING).unwrap();
        
        // Not all drivers default to 32bit color, so explicitly set it to 32bit color
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
        if settings.samples != 0 {
            sdl2::video::gl_set_attribute(
                sdl2::video::GLAttr::GLMultiSampleBuffers, 
                1
            );
            sdl2::video::gl_set_attribute(
                sdl2::video::GLAttr::GLMultiSampleSamples,
                settings.samples as i32
            );
        }

        let window = sdl2::video::Window::new(
            &settings.title,
            sdl2::video::WindowPos::PosCentered,
            sdl2::video::WindowPos::PosCentered,
            settings.size[0] as i32,
            settings.size[1] as i32,
            sdl2::video::OPENGL| sdl2::video::RESIZABLE
        ).unwrap();
        if settings.fullscreen {
            window.set_fullscreen(sdl2::video::FullscreenType::FTTrue);
        }

        // Send text input events.
        sdl2::keyboard::start_text_input();

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers
        gl::load_with(|s| {
            Sdl2Window::get_proc_address(s)
        });

        Sdl2Window {
            exit_on_esc: settings.exit_on_esc,
            should_close: false,
            window: window,
            context: context,
            sdl_context: sdl_context,
            mouse_relative: None,
        }
    }

    /// Returns the address of an OpenGL function if it exist, else returns null pointer.
    pub fn get_proc_address(proc_name: &str) -> *const libc::c_void {
        unsafe {
            transmute(sdl2::video::gl_get_proc_address(proc_name))
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
            sdl2::event::Event::Window { win_event_id: sdl2::event::WindowEventId::Resized, data1: w, data2: h, .. } => {
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
        self.set_mut(CaptureCursor(false));
    }
}

quack! {
    _obj: Sdl2Window[]
    get:
        fn () -> ShouldClose [] { ShouldClose(_obj.should_close) }
        fn () -> Size [] {
            let (w, h) = _obj.window.get_size();
            Size([w as u32, h as u32])
        }
        fn () -> DrawSize [] {
            let (w, h) = _obj.window.get_drawable_size();
            DrawSize([w as u32, h as u32])
        }
        fn () -> Title [] { Title(_obj.window.get_title()) }
        fn () -> ExitOnEsc [] { ExitOnEsc(_obj.exit_on_esc) }
    set:
        fn (val: CaptureCursor) [] {
            sdl2::mouse::set_relative_mouse_mode(val.0)
        }
        fn (val: ShouldClose) [] { _obj.should_close = val.0 }
        fn (val: Title) [] { _obj.window.set_title(&val.0).unwrap() }
        fn (val: ExitOnEsc) [] { _obj.exit_on_esc = val.0 }
    action:
        fn (__: SwapBuffers) -> () [] { _obj.window.gl_swap_window() }
        fn (__: PollEvent) -> Option<Input> [] { _obj.poll_event() }
}

impl Associative for (PollEvent, Sdl2Window) {
    type Type = Input;
}

/// Maps a SDL2 key to piston-input key.
pub fn sdl2_map_key(keycode: sdl2::keycode::KeyCode) -> keyboard::Key {
    use std::num::FromPrimitive;
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
