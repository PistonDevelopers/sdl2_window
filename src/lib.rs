#![deny(missing_docs)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate piston;
extern crate shader_version;
extern crate gl;
extern crate num;

// External crates.
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
    title: String,
    size: Size,
    draw_size: Size,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2.
    pub fn new(settings: WindowSettings) -> Self {
        use sdl2::video::{ GLProfile, gl_attr };

        let sdl_context = sdl2::init().everything().unwrap();
        let opengl = settings.get_maybe_opengl().unwrap_or(OpenGL::_3_2);
        let (major, minor) = opengl.get_major_minor();

        // Not all drivers default to 32bit color, so explicitly set it to 32bit color.
        gl_attr::set_red_size(8);
        gl_attr::set_green_size(8);
        gl_attr::set_blue_size(8);
        gl_attr::set_alpha_size(8);
        gl_attr::set_stencil_size(8);
        gl_attr::set_context_version(major as u8, minor as u8);

        if opengl >= OpenGL::_3_2 {
            gl_attr::set_context_profile(GLProfile::Core);
        }
        if settings.get_samples() != 0 {
            gl_attr::set_multisample_buffers(1);
            gl_attr::set_multisample_samples(settings.get_samples());
        }

        let mut window_builder = sdl_context.window(
            &settings.get_title(),
            settings.get_size().width as u32,
            settings.get_size().height as u32
        );

        let window_builder = window_builder.position_centered()
            .opengl()
            .resizable();

        let window_builder = if settings.get_fullscreen() {
            window_builder.fullscreen()
        } else {
            window_builder
        };

        let window = window_builder.build();

        let window = match window {
            Ok(w) => w,
            Err(_) =>
                if settings.get_samples() != 0 {
                    // Retry without requiring anti-aliasing.
                    gl_attr::set_multisample_buffers(0);
                    gl_attr::set_multisample_samples(0);
                    window_builder.build().unwrap()
                } else {
                    window.unwrap() // Panic.
                }
        };

        // Send text input events.
        sdl2::keyboard::start_text_input();

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers.
        gl::load_with(sdl2::video::gl_get_proc_address);

        if settings.get_vsync() {
            sdl2::video::gl_set_swap_interval(1);
        } else {
            sdl2::video::gl_set_swap_interval(0);
        }

        let mut window = Sdl2Window {
            exit_on_esc: settings.get_exit_on_esc(),
            should_close: false,
            window: window,
            context: context,
            sdl_context: sdl_context,
            mouse_relative: None,
            title: settings.get_title() ,
            size: settings.get_size(),
            draw_size: settings.get_size(),
        };
        window.update_draw_size();
        window
    }

    fn update_draw_size(&mut self) {
        let properties = self.window.properties(&self.sdl_context);
        let (w, h) = properties.get_drawable_size();
        self.draw_size = Size { width: w as u32, height: h as u32 };
    }

    fn poll_event(&mut self) -> Option<Input> {
        // First check for a pending relative mouse move event.
        if let Some((x, y)) = self.mouse_relative {
            self.mouse_relative = None;
            return Some(Input::Move(Motion::MouseRelative(x, y)));
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
                self.size.width = w as u32;
                self.size.height = h as u32;
                self.update_draw_size();
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

impl From<WindowSettings> for Sdl2Window {
    fn from(settings: WindowSettings) -> Sdl2Window {
        Sdl2Window::new(settings)
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
    fn size(&self) -> Size { self.size }
    fn poll_event(&mut self) -> Option<Input> { self.poll_event() }
    fn draw_size(&self) -> Size { self.draw_size }
}

impl AdvancedWindow for Sdl2Window {
    fn get_title(&self) -> String {
        self.title.clone()
    }
    fn set_title(&mut self, value: String) {
        let _ = self.window.properties(&self.sdl_context).set_title(&value);
        self.title = value
    }
    fn get_exit_on_esc(&self) -> bool { self.exit_on_esc }
    fn set_exit_on_esc(&mut self, value: bool) { self.exit_on_esc = value; }
    fn set_capture_cursor(&mut self, value: bool) {
        sdl2::mouse::set_relative_mouse_mode(value);
    }
}

impl OpenGLWindow for Sdl2Window {
    fn get_proc_address(&mut self, proc_name: &str) -> ProcAddress {
        sdl2::video::gl_get_proc_address(proc_name)
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn make_current(&mut self) {
        self.window.gl_make_current(&self.context).unwrap();
    }
}

/// Maps a SDL2 key to piston-input key.
pub fn sdl2_map_key(keycode: sdl2::keycode::KeyCode) -> keyboard::Key {
    num::FromPrimitive::from_u64(keycode as u64).unwrap()
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
