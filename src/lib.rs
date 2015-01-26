#![deny(missing_docs)]
#![allow(unstable)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate shader_version;
extern crate input;
extern crate gl;
extern crate quack;

// External crates.
use std::mem::transmute;
use window::{
    WindowSettings,
    ShouldClose, Size, PollEvent, SwapBuffers,
    CaptureCursor, DrawSize, Title, ExitOnEsc
};
use input::{ keyboard, Button, MouseButton, Input, Motion };
use shader_version::opengl::OpenGL;
use quack::{ ActOn, Action, GetFrom, SetAt, Set };

/// A widow implemented by SDL2 back-end.
pub struct Sdl2Window {
    /// SDL window handle
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    should_close: bool,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2.
    pub fn new(opengl: OpenGL, settings: WindowSettings) -> Sdl2Window {
        sdl2::init(sdl2::INIT_EVERYTHING);
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
            settings.title.as_slice(),
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
        gl::load_with(|s| unsafe {
            transmute(sdl2::video::gl_get_proc_address(s))
        });

        Sdl2Window {
            exit_on_esc: settings.exit_on_esc,
            should_close: false,
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

impl GetFrom for (ShouldClose, Sdl2Window) {
    fn get_from(obj: &Sdl2Window) -> ShouldClose {
        ShouldClose(obj.should_close)
    }
}

impl GetFrom for (Size, Sdl2Window) {
    fn get_from(obj: &Sdl2Window) -> Size {
        let (w, h) = obj.window.get_size();
        Size([w as u32, h as u32])
    }
}

impl ActOn<()> for (SwapBuffers, Sdl2Window) {
    fn act_on(_: SwapBuffers, window: &mut Sdl2Window) {
        window.window.gl_swap_window();
    }
}

impl ActOn<Option<Input>> for (PollEvent, Sdl2Window) {
    fn act_on(
        _: PollEvent, 
        window: &mut Sdl2Window
    ) -> Option<Input> {
        match window.mouse_relative {
            Some((x, y)) => {
                window.mouse_relative = None;
                return Some(Input::Move(Motion::MouseRelative(x, y)));
            }
            None => {}
        }
        match sdl2::event::poll_event() {
            sdl2::event::Event::Quit(_) => {
                window.should_close = true;
            }
            sdl2::event::Event::TextInput(_, _, text) => {
                return Some(Input::Text(text));
            }
            sdl2::event::Event::KeyDown(_, _, key, _, _, repeat) => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                if repeat {
                    return window.action(PollEvent)
                }

                if window.exit_on_esc
                && key == sdl2::keycode::KeyCode::Escape {
                    window.should_close = true;
                } else {
                    return Some(Input::Press(Button::Keyboard(sdl2_map_key(key))));
                }
            }
            sdl2::event::Event::KeyUp(_, _, key, _, _, repeat) => {
                if repeat {
                    return window.action(PollEvent)
                }
                return Some(Input::Release(Button::Keyboard(sdl2_map_key(key))));
            }
            sdl2::event::Event::MouseButtonDown(_, _, _, button, _, _) => {
                return Some(Input::Press(Button::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::Event::MouseButtonUp(_, _, _, button, _, _) => {
                return Some(Input::Release(Button::Mouse(sdl2_map_mouse(button))));
            }
            sdl2::event::Event::MouseMotion(_, _, _, _, x, y, dx, dy) => {
                // Send relative move movement next time.
                window.mouse_relative = Some((dx as f64, dy as f64));
                return Some(Input::Move(Motion::MouseCursor(x as f64, y as f64)));
            },
            sdl2::event::Event::MouseWheel(_, _, _, x, y) => {
                return Some(Input::Move(Motion::MouseScroll(x as f64, y as f64)));
            }
            sdl2::event::Event::Window(_, _, sdl2::event::WindowEventId::Resized, w, h) => {
                return Some(Input::Resize(w as u32, h as u32));
            }
            sdl2::event::Event::Window(_, _, sdl2::event::WindowEventId::FocusGained, _, _) => {
                return Some(Input::Focus(true));
            }
            sdl2::event::Event::Window(_, _, sdl2::event::WindowEventId::FocusLost, _, _) => {
                return Some(Input::Focus(false));
            }
            _ => {}
        }
        None
    }
}

impl SetAt for (CaptureCursor, Sdl2Window) {
    fn set_at(
        CaptureCursor(enabled): 
        CaptureCursor, _window: &mut Sdl2Window
    ) {
        sdl2::mouse::set_relative_mouse_mode(enabled)
    }
}

impl SetAt for (ShouldClose, Sdl2Window) {
    fn set_at(
        ShouldClose(val): ShouldClose, 
        window: &mut Sdl2Window
    ) {
        window.should_close = val;
    }
}

impl GetFrom for (DrawSize, Sdl2Window) {
    fn get_from(obj: &Sdl2Window) -> DrawSize {
        let (w, h) = obj.window.get_drawable_size();
        DrawSize([w as u32, h as u32])
    }
}

impl GetFrom for (Title, Sdl2Window) {
    fn get_from(obj: &Sdl2Window) -> Title {
        Title(obj.window.get_title())
    }
}

impl SetAt for (Title, Sdl2Window) {
    fn set_at(Title(val): Title, window: &mut Sdl2Window) {
        window.window.set_title(val.as_slice());
    }
}

impl GetFrom for (ExitOnEsc, Sdl2Window) {
    fn get_from(obj: &Sdl2Window) -> ExitOnEsc {
        ExitOnEsc(obj.exit_on_esc)
    }
}

impl SetAt for (ExitOnEsc, Sdl2Window) {
    fn set_at(
        ExitOnEsc(val): ExitOnEsc, 
        window: &mut Sdl2Window
    ) {
        window.exit_on_esc = val;
    }
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
