#![deny(missing_docs)]
//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate input;
extern crate shader_version;
extern crate gl;

// External crates.
use window::{
    BuildFromWindowSettings,
    OpenGLWindow,
    ProcAddress,
    Window,
    AdvancedWindow,
    WindowSettings,
    Size,
};
use input::{ keyboard, Button, MouseButton, Input, Motion, JoystickAxisArgs, JoystickButton };

use std::vec::Vec;

pub use shader_version::OpenGL;

struct JoystickState {
    joysticks: Vec<sdl2::joystick::Joystick>,
    subsystem: sdl2::JoystickSubsystem,
}

impl JoystickState {
    fn new(subsystem: sdl2::JoystickSubsystem) -> Self {
        JoystickState {
            joysticks: Vec::new(),
            subsystem: subsystem,
        }
    }
}

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
    /// Video subsystem.
    pub video_subsystem: sdl2::VideoSubsystem,
    joystick_state: Option<JoystickState>,
    should_close: bool,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
    title: String,
    size: Size,
    draw_size: Size,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2. This will initialize SDL and the video subsystem.
    /// You can retrieve both via the public fields on the `Sdl2Window` struct.
    pub fn new(settings: WindowSettings) -> Result<Self, String> {
        let sdl = try!(sdl2::init().map_err(|e| format!("{}", e)));
        let video_subsystem = try!(sdl.video()
            .map_err(|e| format!("{}", e)));
        let mut window = try!(Self::with_subsystem(video_subsystem, settings));
        // Enable joysticks by default.
        try!(window.init_joysticks().map_err(|e| e));
        Ok(window)
    }

    /// Creates a window with the supplied SDL Video subsystem.
    pub fn with_subsystem(video_subsystem: sdl2::VideoSubsystem, settings: WindowSettings) -> Result<Self, String> {
        use sdl2::video::GLProfile;

        let sdl_context = video_subsystem.sdl();
        let opengl = settings.get_maybe_opengl().unwrap_or(OpenGL::V3_2);
        let (major, minor) = opengl.get_major_minor();

        {
            let gl_attr = video_subsystem.gl_attr();

            // Not all drivers default to 32bit color, so explicitly set it to 32bit color.
            gl_attr.set_red_size(8);
            gl_attr.set_green_size(8);
            gl_attr.set_blue_size(8);
            gl_attr.set_alpha_size(8);
            gl_attr.set_stencil_size(8);
            gl_attr.set_context_version(major as u8, minor as u8);
            gl_attr.set_framebuffer_srgb_compatible(settings.get_srgb());
        }

        if opengl >= OpenGL::V3_2 {
            video_subsystem.gl_attr().set_context_profile(GLProfile::Core);
        }
        if settings.get_samples() != 0 {
            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_multisample_buffers(1);
            gl_attr.set_multisample_samples(settings.get_samples());
        }

        let mut window_builder = video_subsystem.window(
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
                    let gl_attr = video_subsystem.gl_attr();
                    gl_attr.set_multisample_buffers(0);
                    gl_attr.set_multisample_samples(0);
                    try!(window_builder.build().map_err(|e| format!("{}", e)))
                } else {
                    try!(window.map_err(|e| format!("{}", e)))
                }
        };

        // Send text input events.
        video_subsystem.text_input().start();

        let context = try!(window.gl_create_context()
            .map_err(|e| format!("{}", e)));

        // Load the OpenGL function pointers.
        gl::load_with(|name| video_subsystem.gl_get_proc_address(name) as *const _);

        if settings.get_vsync() {
            video_subsystem.gl_set_swap_interval(1);
        } else {
            video_subsystem.gl_set_swap_interval(0);
        }

        let mut window = Sdl2Window {
            exit_on_esc: settings.get_exit_on_esc(),
            should_close: false,
            window: window,
            context: context,
            sdl_context: sdl_context,
            video_subsystem: video_subsystem,
            joystick_state: None,
            mouse_relative: None,
            title: settings.get_title() ,
            size: settings.get_size(),
            draw_size: settings.get_size(),
        };
        window.update_draw_size();
        Ok(window)
    }

    /// Initialize the joystick subsystem. Required before joystick input
    /// events will be returned. Returns the number available or error.
    pub fn init_joysticks(&mut self) -> Result<u32, String> {
        let subsystem = try!(self.sdl_context.joystick().map_err(|e| format!("{}", e)));
        let mut state = JoystickState::new(subsystem);
        let available = try!(state.subsystem.num_joysticks().map_err(|e| format!("{}", e)));

        // Open all the joysticks
        for id in 0..available {
            match state.subsystem.open(id) {
                Ok(c) => { state.joysticks.push(c) },
                Err(e) => return Err(format!("{}", e)),
            }
        }

        self.joystick_state = Some(state);

        Ok(available)
    }

    fn update_draw_size(&mut self) {
        let (w, h) = self.window.drawable_size();
        self.draw_size = Size { width: w as u32, height: h as u32 };
    }

    fn poll_event(&mut self) -> Option<Input> {
        use sdl2::event::{ Event, WindowEventId };

        // First check for a pending relative mouse move event.
        if let Some((x, y)) = self.mouse_relative {
            self.mouse_relative = None;
            return Some(Input::Move(Motion::MouseRelative(x, y)));
        }

        // Even though we create a new EventPump each time we poll an event
        // this should not be a problem since it only contains phantom data
        // and therefore should actually not have any overhead.
        let event = match self.sdl_context.event_pump().unwrap().poll_event() {
            Some( ev ) => ev,
            None => return None
        };
        match event {
            Event::Quit{..} => {
                self.should_close = true;
            }
            Event::TextInput { text, .. } => {
                return Some(Input::Text(text));
            }
            Event::KeyDown { keycode: Some(key), repeat, ..} => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                if repeat {
                    return self.poll_event()
                }

                if self.exit_on_esc
                && key == sdl2::keyboard::Keycode::Escape {
                    self.should_close = true;
                } else {
                    return Some(Input::Press(Button::Keyboard(sdl2_map_key(key))));
                }
            }
            Event::KeyUp { keycode: Some(key), repeat, .. } => {
                if repeat {
                    return self.poll_event()
                }
                return Some(Input::Release(Button::Keyboard(sdl2_map_key(key))));
            }
            Event::MouseButtonDown { mouse_btn: button, .. } => {
                return Some(Input::Press(Button::Mouse(sdl2_map_mouse(button))));
            }
            Event::MouseButtonUp { mouse_btn: button, .. } => {
                return Some(Input::Release(Button::Mouse(sdl2_map_mouse(button))));
            }
            Event::MouseMotion { x, y, xrel: dx, yrel: dy, .. } => {
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64));
                return Some(Input::Move(Motion::MouseCursor(x as f64, y as f64)));
            },
            Event::MouseWheel { x, y, .. } => {
                return Some(Input::Move(Motion::MouseScroll(x as f64, y as f64)));
            }
            Event::JoyAxisMotion{ which, axis_idx, value: val, .. } => {
                // Axis motion is an absolute value in the range
                // [-32768, 32767]. Normalize it down to a float.
                use std::i16::MAX;
                let normalized_value = val as f64 / MAX as f64;
                return Some(Input::Move(Motion::JoystickAxis(JoystickAxisArgs::new(which, axis_idx, normalized_value))));
            }
            Event::JoyButtonDown{ which, button_idx, .. } => {
                return Some(Input::Press(Button::Joystick(JoystickButton::new(which, button_idx))))
            },
            Event::JoyButtonUp{ which, button_idx, .. } => {
                return Some(Input::Release(Button::Joystick(JoystickButton::new(which, button_idx))))
            },
            Event::Window {
                win_event_id: sdl2::event::WindowEventId::Resized, data1: w, data2: h, .. } => {
                self.size.width = w as u32;
                self.size.height = h as u32;
                self.update_draw_size();
                return Some(Input::Resize(w as u32, h as u32));
            }
            Event::Window { win_event_id: WindowEventId::FocusGained, .. } => {
                return Some(Input::Focus(true));
            }
            Event::Window { win_event_id: WindowEventId::FocusLost, .. } => {
                return Some(Input::Focus(false));
            }
            Event::Window { win_event_id: WindowEventId::Enter, .. } => {
                return Some(Input::Cursor(true));
            }
            Event::Window { win_event_id: WindowEventId::Leave, .. } => {
                return Some(Input::Cursor(false));
            }
            _ => {}
        }
        None
    }
}

impl BuildFromWindowSettings for Sdl2Window {
    fn build_from_window_settings(settings: WindowSettings)
    -> Result<Self, String> {
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
    fn set_should_close(&mut self, value: bool) { self.should_close = value; }
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
        let _ = self.window.set_title(&value);
        self.title = value
    }
    fn get_exit_on_esc(&self) -> bool { self.exit_on_esc }
    fn set_exit_on_esc(&mut self, value: bool) { self.exit_on_esc = value; }
    fn set_capture_cursor(&mut self, value: bool) {
        self.sdl_context.mouse().set_relative_mouse_mode(value);
    }
}

impl OpenGLWindow for Sdl2Window {
    fn get_proc_address(&mut self, proc_name: &str) -> ProcAddress {
        self.video_subsystem.gl_get_proc_address(proc_name) as *const _
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn make_current(&mut self) {
        self.window.gl_make_current(&self.context).unwrap();
    }
}

/// Maps a SDL2 key to piston-input key.
pub fn sdl2_map_key(keycode: sdl2::keyboard::Keycode) -> keyboard::Key {
    (keycode as u32).into()
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
