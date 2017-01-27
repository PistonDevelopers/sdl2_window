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
    Position,
};
use input::{
    keyboard,
    Button,
    MouseButton,
    Input,
    Motion,
    CloseArgs,
    ControllerAxisArgs,
    ControllerButton,
    Touch,
    TouchArgs,
};

use std::vec::Vec;
use std::time::Duration;

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

/// A window implemented by SDL2 back-end.
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
    // Stores relative coordinates to emit on next poll.
    mouse_relative: Option<(f64, f64)>,
    // Whether the cursor is captured.
    is_capturing_cursor: bool,
    // Used to ignore relative events when warping mouse
    // to center of window.
    ignore_relative_event: Option<(i32, i32)>,
    exit_on_esc: bool,
    title: String,
    size: Size,
    draw_size: Size,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2. This will initialize SDL and the video subsystem.
    /// You can retrieve both via the public fields on the `Sdl2Window` struct.
    pub fn new(settings: &WindowSettings) -> Result<Self, String> {
        let sdl = try!(sdl2::init().map_err(|e| format!("{}", e)));
        let video_subsystem = try!(sdl.video()
            .map_err(|e| format!("{}", e)));
        let mut window = try!(Self::with_subsystem(video_subsystem, settings));
        if settings.get_controllers() {
            try!(window.init_joysticks().map_err(|e| e));
        }
        Ok(window)
    }

    /// Creates a window with the supplied SDL Video subsystem.
    pub fn with_subsystem(video_subsystem: sdl2::VideoSubsystem, settings: &WindowSettings) -> Result<Self, String> {
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
            .opengl();

        let window_builder = if settings.get_resizable() {
            window_builder.resizable()
        } else {
            window_builder
        };

        let window_builder = if settings.get_decorated() {
            window_builder
        } else {
            window_builder.borderless()
        };

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
            is_capturing_cursor: false,
            ignore_relative_event: None,
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

    fn wait_event(&mut self) -> Input {
        if let Some(event) = self.check_pending_event() {
            return event;
        };
        loop {
            let sdl_event = self.sdl_context.event_pump().unwrap().wait_event();
            if let Some(event) = self.handle_event(Some(sdl_event)) {
                return event;
            }
        }
    }

    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Input> {
        if let Some(event) = self.check_pending_event() {
            return Some(event);
        };
        let timeout_ms = timeout.as_secs() as u32 * 1000 + (timeout.subsec_nanos() / 1_000_000);
        let sdl_event = self.sdl_context.event_pump().unwrap().wait_event_timeout(timeout_ms);
        self.handle_event(sdl_event)
    }

    fn poll_event(&mut self) -> Option<Input> {
        if let Some(event) = self.check_pending_event() {
            return Some(event);
        };
        // Even though we create a new EventPump each time we poll an event
        // this should not be a problem since it only contains phantom data
        // and therefore should actually not have any overhead.
        let sdl_event = self.sdl_context.event_pump().unwrap().poll_event();
        self.handle_event(sdl_event)
    }

    fn check_pending_event(&mut self) -> Option<Input> {
        // First check for a pending relative mouse move event.
        if let Some((x, y)) = self.mouse_relative {
            self.mouse_relative = None;
            return Some(Input::Move(Motion::MouseRelative(x, y)));
        }
        None
    }

    fn handle_event(&mut self, sdl_event: Option<sdl2::event::Event>) -> Option<Input> {
        use sdl2::event::{ Event, WindowEvent };
        let event = match sdl_event {
            Some( ev ) => {
                if let Event::MouseMotion { xrel, yrel, .. } = ev {
                    // Ignore a specific mouse motion event caused by
                    // change of coordinates when warping the cursor
                    // to the center.
                    if Some((xrel, yrel)) == self.ignore_relative_event {
                        self.ignore_relative_event = None;
                        return None;
                    }
                }
                ev
            }
            None => {
                // Wait until event queue is empty to reduce
                // risk of error in order.
                if self.is_capturing_cursor {
                    self.fake_capture();
                }
                return None
            }
        };
        match event {
            Event::Quit{..} => {
                self.should_close = true;
                return Some(Input::Close(CloseArgs));
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
                if self.is_capturing_cursor {
                    // Skip normal mouse movement and emit relative motion only.
                    return Some(Input::Move(Motion::MouseRelative(dx as f64, dy as f64)));
                }
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64));
                return Some(Input::Move(Motion::MouseCursor(x as f64, y as f64)));
            }
            Event::MouseWheel { x, y, .. } => {
                return Some(Input::Move(Motion::MouseScroll(x as f64, y as f64)));
            }
            Event::JoyAxisMotion{ which, axis_idx, value: val, .. } => {
                // Axis motion is an absolute value in the range
                // [-32768, 32767]. Normalize it down to a float.
                use std::i16::MAX;
                let normalized_value = val as f64 / MAX as f64;
                return Some(Input::Move(Motion::ControllerAxis(ControllerAxisArgs::new(
                    which, axis_idx, normalized_value))));
            }
            Event::JoyButtonDown{ which, button_idx, .. } => {
                return Some(Input::Press(Button::Controller(ControllerButton::new(
                    which, button_idx))))
            }
            Event::JoyButtonUp{ which, button_idx, .. } => {
                return Some(Input::Release(Button::Controller(ControllerButton::new(
                    which, button_idx))))
            }
            Event::FingerDown { touch_id, finger_id, x, y, pressure, .. } => {
                return Some(Input::Move(Motion::Touch(TouchArgs::new(
                    touch_id, finger_id, [x as f64, y as f64], pressure as f64, Touch::Start))))
            }
            Event::FingerMotion { touch_id, finger_id, x, y, pressure, .. } => {
                return Some(Input::Move(Motion::Touch(TouchArgs::new(
                    touch_id, finger_id, [x as f64, y as f64], pressure as f64, Touch::Move))))
            }
            Event::FingerUp { touch_id, finger_id, x, y, pressure, .. } => {
                return Some(Input::Move(Motion::Touch(TouchArgs::new(
                    touch_id, finger_id, [x as f64, y as f64], pressure as f64, Touch::End))))
            }
            Event::Window {
                win_event: sdl2::event::WindowEvent::Resized(w, h), .. } => {
                self.size.width = w as u32;
                self.size.height = h as u32;
                self.update_draw_size();
                return Some(Input::Resize(w as u32, h as u32));
            }
            Event::Window { win_event: WindowEvent::FocusGained, .. } => {
                return Some(Input::Focus(true));
            }
            Event::Window { win_event: WindowEvent::FocusLost, .. } => {
                return Some(Input::Focus(false));
            }
            Event::Window { win_event: WindowEvent::Enter, .. } => {
                return Some(Input::Cursor(true));
            }
            Event::Window { win_event: WindowEvent::Leave, .. } => {
                return Some(Input::Cursor(false));
            }
            _ => {}
        }
        None
    }

    fn fake_capture(&mut self) {
        // Fake capturing of cursor.
        let cx = (self.size.width / 2) as i32;
        let cy = (self.size.height / 2) as i32;
        let s = self.sdl_context.event_pump().unwrap().mouse_state();
        let dx = cx - s.x();
        let dy = cy - s.y();
        if dx != 0 || dy != 0 {
            self.ignore_relative_event = Some((dx, dy));
            self.sdl_context.mouse().warp_mouse_in_window(
                &self.window, cx as i32, cy as i32
            );
        }
    }
}

impl BuildFromWindowSettings for Sdl2Window {
    fn build_from_window_settings(settings: &WindowSettings) -> Result<Self, String> {
        Sdl2Window::new(settings)
    }
}

impl Drop for Sdl2Window {
    fn drop(&mut self) {
        self.set_capture_cursor(false);
    }
}

impl Window for Sdl2Window {
    fn should_close(&self) -> bool { self.should_close }
    fn set_should_close(&mut self, value: bool) { self.should_close = value; }
    fn swap_buffers(&mut self) { self.window.gl_swap_window(); }
    fn size(&self) -> Size { self.size }
    fn wait_event(&mut self) -> Input { self.wait_event() }
    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Input> { self.wait_event_timeout(timeout) }
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
        // Normally it should call `.set_relative_mouse_mode(value)`,
        // but since it does not emit relative mouse events,
        // we have to fake it by hiding the cursor and warping it
        // back to the center of the window.
        self.is_capturing_cursor = value;
        self.sdl_context.mouse().show_cursor(!value);
        if value {
            // Move cursor to center of window now,
            // to get right relative mouse motion to ignore.
            self.fake_capture();
        }
    }
    fn show(&mut self) { self.window.show(); }
    fn hide(&mut self) { self.window.hide(); }
    fn get_position(&self) -> Option<Position> {
        let (x, y) = self.window.position();
        Some(Position { x: x, y: y })
    }
    fn set_position<P: Into<Position>>(&mut self, pos: P) {
        use sdl2::video::WindowPos;

        let pos: Position = pos.into();
        self.window.set_position(WindowPos::Positioned(pos.x), WindowPos::Positioned(pos.y));
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
pub fn sdl2_map_mouse(button: sdl2::mouse::MouseButton) -> MouseButton {
    use sdl2::mouse::MouseButton as MB;

    match button {
        MB::Left => MouseButton::Left,
        MB::Right => MouseButton::Right,
        MB::Middle => MouseButton::Middle,
        MB::X1 => MouseButton::X1,
        MB::X2 => MouseButton::X2,
        MB::Unknown => MouseButton::Unknown,
    }
}
