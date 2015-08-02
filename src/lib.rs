#![deny(missing_docs)]

//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate input;
extern crate shader_version;
extern crate gl;
extern crate num;

// External crates.
use window::{
    OpenGLWindow,
    ProcAddress,
    Window,
    AdvancedWindow,
    WindowSettings,
    Size,
};
use input::{ keyboard, Button, MouseButton, Input, Motion };

use sdl2::{
    VideoSubsystem,
    EventSubsystem
};

pub use shader_version::OpenGL;

/// A widow implemented by SDL2 back-end.
pub struct Sdl2Window {
    /// SDL window handle.
    pub window: sdl2::video::Window,
    /// Allow dead code because this keeps track of the OpenGL context.
    /// Will be released on drop.
    #[allow(dead_code)]
    pub context: sdl2::video::GLContext,
    /// The SDL video subsystem.
    pub video_subsystem: sdl2::VideoSubsystem,
    /// The SDL events subsystem.
    pub event_subsystem: sdl2::EventSubsystem,
    should_close: bool,
    mouse_relative: Option<(f64, f64)>,
    exit_on_esc: bool,
    title: String,
    size: Size,
    draw_size: Size,
}

impl Sdl2Window {
    /// Creates a new Piston window for SDL2. This will initialize SDL and the
    /// video and event subsystems.
    pub fn new(settings: WindowSettings) -> Result<Self, String> {
        let sdl_context = try!(sdl2::init());
        let video = try!(sdl_context.video());
        let mut event = try!(sdl_context.event());
        Self::new_with_subsystems(video, &mut event, settings)
    }

    /// Creates a new window with the supplied video and event subsystems.
    ///
    /// ```
    /// let sdl = sdl2::Sdl::new().unwrap();
    /// let video = sdl.video().unwrap();
    /// let event = sdl.event().unwrap();
    /// let window = Sdl2Window::new_with_subsystems(
    ///     video.clone(), &mut event,
    ///     WindowSettings::new("example", (640, 480)));
    /// ```
    pub fn new_with_subsystems(video: VideoSubsystem, event: &mut EventSubsystem,
        settings: WindowSettings) -> Result<Self, String> {
        use sdl2::video::{ GLProfile };

        let opengl = settings.get_maybe_opengl().unwrap_or(OpenGL::V3_2);
        let (major, minor) = opengl.get_major_minor();

        // Not all drivers default to 32bit color, so explicitly set it to 32bit color.

        let gl_attr = video.gl_attr();

        gl_attr.set_red_size(8);
        gl_attr.set_green_size(8);
        gl_attr.set_blue_size(8);
        gl_attr.set_alpha_size(8);
        gl_attr.set_stencil_size(8);
        gl_attr.set_context_version(major as u8, minor as u8);

        if opengl >= OpenGL::V3_2 {
            gl_attr.set_context_profile(GLProfile::Core);
        }
        if settings.get_samples() != 0 {
            gl_attr.set_multisample_buffers(1);
            gl_attr.set_multisample_samples(settings.get_samples());
        }

        let mut window_builder = video.window(
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
                    gl_attr.set_multisample_buffers(0);
                    gl_attr.set_multisample_samples(0);
                    window_builder.build().unwrap()
                } else {
                    window.unwrap() // Panic.
                }
        };

        // Send text input events.
        let text_input_util = video.text_input();
        text_input_util.start();

        let context = window.gl_create_context().unwrap();

        // Load the OpenGL function pointers.
        gl::load_with(|a| {
            video.gl_get_proc_address(a)
        });

        if settings.get_vsync() {
            video.gl_set_swap_interval(1);
        } else {
            video.gl_set_swap_interval(0);
        }

        let mut window = Sdl2Window {
            exit_on_esc: settings.get_exit_on_esc(),
            should_close: false,
            window: window,
            context: context,
            video_subsystem: video.clone(),
            event_subsystem: event.clone(),
            mouse_relative: None,
            title: settings.get_title() ,
            size: settings.get_size(),
            draw_size: settings.get_size(),
        };
        window.update_draw_size();
        Ok(window)
    }

    fn update_draw_size(&mut self) {
        let (w, h) = self.window.get_drawable_size();
        self.draw_size = Size { width: w as u32, height: h as u32 };
    }

    fn poll_event(&mut self) -> Result<Option<Input>, String> {
        // First check for a pending relative mouse move event.
        if let Some((x, y)) = self.mouse_relative {
            self.mouse_relative = None;
            return Ok(Some(Input::Move(Motion::MouseRelative(x, y))));
        }

        // Even though we create a new EventPump each time we poll an event
        // this should not be a problem since it only contains phantom data
        // and therefore should actually not have any overhead.
        let sdl = self.video_subsystem.sdl();
        let mut event_pump = match sdl.event_pump() {
            Ok(e) => e,
            Err(s) => return Err(s)
        };

        let event = match event_pump.poll_event() {
            Some( ev ) => ev,
            None => return Ok(None)
        };
        match event {
            sdl2::event::Event::Quit{..} => {
                self.should_close = true;
            }
            sdl2::event::Event::TextInput { text, .. } => {
                return Ok(Some(Input::Text(text)));
            }
            sdl2::event::Event::KeyDown { keycode: Some(key), repeat, ..} => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                if repeat {
                    return self.poll_event()
                }

                if self.exit_on_esc
                && key == sdl2::keyboard::Keycode::Escape {
                    self.should_close = true;
                } else {
                    return Ok(Some(Input::Press(Button::Keyboard(sdl2_map_key(key)))));
                }
            }
            sdl2::event::Event::KeyUp { keycode: Some(key), repeat, .. } => {
                if repeat {
                    return self.poll_event()
                }
                return Ok(Some(Input::Release(Button::Keyboard(sdl2_map_key(key)))));
            }
            sdl2::event::Event::MouseButtonDown { mouse_btn: button, .. } => {
                return Ok(Some(Input::Press(Button::Mouse(sdl2_map_mouse(button)))));
            }
            sdl2::event::Event::MouseButtonUp { mouse_btn: button, .. } => {
                return Ok(Some(Input::Release(Button::Mouse(sdl2_map_mouse(button)))));
            }
            sdl2::event::Event::MouseMotion { x, y, xrel: dx, yrel: dy, .. } => {
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64));
                return Ok(Some(Input::Move(Motion::MouseCursor(x as f64, y as f64))));
            },
            sdl2::event::Event::MouseWheel { x, y, .. } => {
                return Ok(Some(Input::Move(Motion::MouseScroll(x as f64, y as f64))));
            }
            sdl2::event::Event::Window {
                win_event_id: sdl2::event::WindowEventId::Resized, data1: w, data2: h, .. } => {
                self.size.width = w as u32;
                self.size.height = h as u32;
                self.update_draw_size();
                return Ok(Some(Input::Resize(w as u32, h as u32)));
            }
            sdl2::event::Event::Window { win_event_id: sdl2::event::WindowEventId::FocusGained, .. } => {
                return Ok(Some(Input::Focus(true)));
            }
            sdl2::event::Event::Window { win_event_id: sdl2::event::WindowEventId::FocusLost, .. } => {
                return Ok(Some(Input::Focus(false)));
            }
            _ => {}
        }
        Ok(None)
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
    fn poll_event(&mut self) -> Option<Input> { self.poll_event().unwrap_or(None) }
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
        let sdl = self.video_subsystem.sdl();
        let mouse_util = sdl.mouse();
        mouse_util.set_relative_mouse_mode(value);
    }
}

impl OpenGLWindow for Sdl2Window {
    fn get_proc_address(&mut self, proc_name: &str) -> ProcAddress {
        self.video_subsystem.gl_get_proc_address(proc_name)
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
