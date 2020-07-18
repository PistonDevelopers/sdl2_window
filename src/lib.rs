#![deny(missing_docs)]
//! A SDL2 window back-end for the Piston game engine.

extern crate sdl2;
extern crate window;
extern crate input;
extern crate shader_version;
extern crate gl;

// External crates.
use window::{BuildFromWindowSettings, OpenGLWindow, ProcAddress, Window, AdvancedWindow,
             WindowSettings, Size, Position, Api, UnsupportedGraphicsApiError};
use input::{keyboard, Button, ButtonArgs, ButtonState, MouseButton, Input, Motion, CloseArgs,
            ControllerAxisArgs, ControllerButton, Touch, TouchArgs, ControllerHat, TimeStamp,
            ResizeArgs, Event};
use input::HatState as PistonHat;
use sdl2::joystick::HatState;

use std::vec::Vec;
use std::time::Duration;
use std::error::Error;

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
    automatic_close: bool,
    // Stores relative coordinates to emit on next poll.
    mouse_relative: Option<(f64, f64, TimeStamp)>,
    // Whether the cursor is captured.
    is_capturing_cursor: bool,
    // Used to ignore relative events when warping mouse
    // to center of window.
    ignore_relative_event: Option<(i32, i32)>,
    exit_on_esc: bool,
    title: String,
}

impl Sdl2Window {
    /// Creates a new game window for SDL2. This will initialize SDL and the video subsystem.
    /// You can retrieve both via the public fields on the `Sdl2Window` struct.
    pub fn new(settings: &WindowSettings) -> Result<Self, Box<dyn Error>> {
        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        Ok(Self::with_subsystem(video_subsystem, settings)?)
    }

    /// Creates a window with the supplied SDL Video subsystem.
    pub fn with_subsystem(video_subsystem: sdl2::VideoSubsystem,
                          settings: &WindowSettings)
                          -> Result<Self, Box<dyn Error>> {
        use sdl2::video::GLProfile;

        let sdl_context = video_subsystem.sdl();
        let api = settings.get_maybe_graphics_api().unwrap_or(Api::opengl(3, 2));
        if api.api != "OpenGL" {
            return Err(UnsupportedGraphicsApiError {
                found: api.api,
                expected: vec!["OpenGL".into()],
            }.into());
        }

        {
            let gl_attr = video_subsystem.gl_attr();

            // Not all drivers default to 32bit color, so explicitly set it to 32bit color.
            gl_attr.set_red_size(8);
            gl_attr.set_green_size(8);
            gl_attr.set_blue_size(8);
            gl_attr.set_alpha_size(8);
            gl_attr.set_stencil_size(8);
            gl_attr.set_context_version(api.major as u8, api.minor as u8);
            gl_attr.set_framebuffer_srgb_compatible(settings.get_srgb());
        }

        if api >= Api::opengl(3, 2) {
            video_subsystem.gl_attr().set_context_profile(GLProfile::Core);
        }
        if settings.get_samples() != 0 {
            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_multisample_buffers(1);
            gl_attr.set_multisample_samples(settings.get_samples());
        }

        let mut window_builder = video_subsystem.window(&settings.get_title(),
                                                        settings.get_size().width as u32,
                                                        settings.get_size().height as u32);

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
            Err(_) => {
                if settings.get_samples() != 0 {
                    // Retry without requiring anti-aliasing.
                    let gl_attr = video_subsystem.gl_attr();
                    gl_attr.set_multisample_buffers(0);
                    gl_attr.set_multisample_samples(0);
                    window_builder.build().map_err(|e| format!("{}", e))?
                } else {
                    window.map_err(|e| format!("{}", e))?
                }
            }
        };

        // Send text input events.
        video_subsystem.text_input().start();

        let context = window.gl_create_context()
            .map_err(|e| format!("{}", e))?;

        // Load the OpenGL function pointers.
        gl::load_with(|name| video_subsystem.gl_get_proc_address(name) as *const _);

        if settings.get_vsync() {
            video_subsystem.gl_set_swap_interval(1)?;
        } else {
            video_subsystem.gl_set_swap_interval(0)?;
        }

        let mut window = Sdl2Window {
            exit_on_esc: settings.get_exit_on_esc(),
            should_close: false,
            automatic_close: settings.get_automatic_close(),
            is_capturing_cursor: false,
            ignore_relative_event: None,
            window: window,
            context: context,
            sdl_context: sdl_context,
            video_subsystem: video_subsystem,
            joystick_state: None,
            mouse_relative: None,
            title: settings.get_title(),
        };
        if settings.get_controllers() {
            window.init_joysticks()?;
        }
        if settings.get_transparent() {
            let _ = window.window.set_opacity(0.0);
        }
        Ok(window)
    }

    /// Initialize the joystick subsystem. Required before joystick input
    /// events will be returned. Returns the number available or error.
    pub fn init_joysticks(&mut self) -> Result<u32, String> {
        let subsystem = self.sdl_context.joystick().map_err(|e| format!("{}", e))?;
        let mut state = JoystickState::new(subsystem);
        let available = state.subsystem.num_joysticks().map_err(|e| format!("{}", e))?;

        // Open all the joysticks
        for id in 0..available {
            match state.subsystem.open(id) {
                Ok(c) => state.joysticks.push(c),
                Err(e) => return Err(format!("{}", e)),
            }
        }

        self.joystick_state = Some(state);

        Ok(available)
    }

    fn wait_event(&mut self) -> Event {
        loop {
            if let Some(event) = self.check_pending_event() {
                return event;
            };
            let sdl_event = self.sdl_context.event_pump().unwrap().wait_event();
            let mut unknown = false;
            if let Some(event) = self.handle_event(Some(sdl_event), &mut unknown) {
                return event;
            }
        }
    }

    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Event> {
        let event = self.check_pending_event();
        if event.is_some() {
            return event;
        };

        let timeout_ms = timeout.as_secs() as u32 * 1000 + (timeout.subsec_nanos() / 1_000_000);
        let sdl_event = self.sdl_context.event_pump().unwrap().wait_event_timeout(timeout_ms);

        let mut unknown = false;
        let event = self.handle_event(sdl_event, &mut unknown);
        if unknown { self.poll_event() } else { event }
    }

    fn poll_event(&mut self) -> Option<Event> {
        // Loop for ignoring unknown events.
        loop {
            let event = self.check_pending_event();
            if event.is_some() {
                return event;
            };

            // Even though we create a new EventPump each time we poll an event
            // this should not be a problem since it only contains phantom data
            // and therefore should actually not have any overhead.
            let sdl_event = self.sdl_context.event_pump().unwrap().poll_event();
            let mut unknown = false;
            let event = self.handle_event(sdl_event, &mut unknown);
            if unknown {
                continue;
            };
            return event;
        }
    }

    fn check_pending_event(&mut self) -> Option<Event> {
        // First check for a pending relative mouse move event.
        if let Some((x, y, timestamp)) = self.mouse_relative {
            self.mouse_relative = None;
            return Some(input::Event::Input(
                Input::Move(Motion::MouseRelative([x, y])), Some(timestamp)));
        }
        None
    }

    fn handle_event(&mut self,
                    sdl_event: Option<sdl2::event::Event>,
                    unknown: &mut bool)
                    -> Option<Event> {
        use sdl2::event::{Event, WindowEvent};
        let event = match sdl_event {
            Some(ev) => {
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
                return None;
            }
        };
        match event {
            Event::Quit { timestamp, .. } => {
                if self.automatic_close {
                    self.should_close = true;
                }
                return Some(input::Event::Input(Input::Close(CloseArgs), Some(timestamp)));
            }
            Event::TextInput { text, timestamp, .. } => {
                return Some(input::Event::Input(Input::Text(text), Some(timestamp)));
            }
            Event::KeyDown { keycode: Some(key), repeat, scancode, timestamp, .. } => {
                // SDL2 repeats the key down event.
                // If the event is the same as last one, ignore it.
                if repeat {
                    return self.poll_event();
                }

                if self.exit_on_esc && key == sdl2::keyboard::Keycode::Escape {
                    self.should_close = true;
                } else {
                    return Some(input::Event::Input(Input::Button(ButtonArgs {
                        state: ButtonState::Press,
                        button: Button::Keyboard(sdl2_map_key(key)),
                        scancode: scancode.map(|scode| scode as i32),
                    }), Some(timestamp)));
                }
            }
            Event::KeyUp { keycode: Some(key), repeat, scancode, timestamp, .. } => {
                if repeat {
                    return self.poll_event();
                }
                return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Release,
                    button: Button::Keyboard(sdl2_map_key(key)),
                    scancode: scancode.map(|scode| scode as i32),
                }), Some(timestamp)));
            }
            Event::MouseButtonDown { mouse_btn: button, timestamp, .. } => {
                return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Press,
                    button: Button::Mouse(sdl2_map_mouse(button)),
                    scancode: None,
                }), Some(timestamp)));
            }
            Event::MouseButtonUp { mouse_btn: button, timestamp, .. } => {
                return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Release,
                    button: Button::Mouse(sdl2_map_mouse(button)),
                    scancode: None,
                }), Some(timestamp)));
            }
            Event::MouseMotion { x, y, xrel: dx, yrel: dy, timestamp, .. } => {
                if self.is_capturing_cursor {
                    // Skip normal mouse movement and emit relative motion only.
                    return Some(input::Event::Input(
                        Input::Move(Motion::MouseRelative([dx as f64, dy as f64])),
                        Some(timestamp)));
                }
                // Send relative move movement next time.
                self.mouse_relative = Some((dx as f64, dy as f64, timestamp));
                return Some(input::Event::Input(
                    Input::Move(Motion::MouseCursor([x as f64, y as f64])),
                    Some(timestamp)));
            }
            Event::MouseWheel { x, y, timestamp, .. } => {
                return Some(input::Event::Input(
                    Input::Move(Motion::MouseScroll([x as f64, y as f64])), Some(timestamp)));
            }
            Event::JoyAxisMotion { which, axis_idx, value: val, timestamp, .. } => {
                // Axis motion is an absolute value in the range
                // [-32768, 32767]. Normalize it down to a float.
                use std::i16::MAX;
                let normalized_value = val as f64 / MAX as f64;
                return Some(input::Event::Input(Input::Move(
                    Motion::ControllerAxis(ControllerAxisArgs::new(
                    which, axis_idx, normalized_value))), Some(timestamp)));
            }
            Event::JoyButtonDown { which, button_idx, timestamp, .. } => {
                return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Press,
                    button: Button::Controller(ControllerButton::new(which, button_idx)),
                    scancode: None,
                }), Some(timestamp)))
            }
            Event::JoyButtonUp { which, button_idx, timestamp, .. } => {
                return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Release,
                    button: Button::Controller(ControllerButton::new(which, button_idx)),
                    scancode: None,
                }), Some(timestamp)))
            }
            Event::JoyHatMotion { which, hat_idx, state, timestamp, .. } => {
              let state = match state {
                HatState::Centered => PistonHat::Centered,
                HatState::Up => PistonHat::Up,
                HatState::Right => PistonHat::Right,
                HatState::Down => PistonHat::Down,
                HatState::Left => PistonHat::Left,
                HatState::RightUp => PistonHat::RightUp,
                HatState::RightDown => PistonHat::RightDown,
                HatState::LeftUp => PistonHat::LeftUp,
                HatState::LeftDown => PistonHat::LeftDown,
              };
              return Some(input::Event::Input(Input::Button(ButtonArgs {
                    state: ButtonState::Release,
                    button: Button::Hat(ControllerHat::new(which, hat_idx, state)),
                    scancode: None,
                }), Some(timestamp)))
            }
            Event::FingerDown { touch_id, finger_id, x, y, pressure, timestamp, .. } => {
                return Some(input::Event::Input(Input::Move(Motion::Touch(TouchArgs::new(touch_id,
                                                                     finger_id,
                                                                     [x as f64, y as f64],
                                                                     pressure as f64,
                                                                     Touch::Start))),
                             Some(timestamp)))
            }
            Event::FingerMotion { touch_id, finger_id, x, y, pressure, timestamp, .. } => {
                return Some(input::Event::Input(Input::Move(Motion::Touch(TouchArgs::new(touch_id,
                                                                     finger_id,
                                                                     [x as f64, y as f64],
                                                                     pressure as f64,
                                                                     Touch::Move))),
                             Some(timestamp)))
            }
            Event::FingerUp { touch_id, finger_id, x, y, pressure, timestamp, .. } => {
                return Some(input::Event::Input(Input::Move(Motion::Touch(TouchArgs::new(touch_id,
                                                                     finger_id,
                                                                     [x as f64, y as f64],
                                                                     pressure as f64,
                                                                     Touch::End))),
                             Some(timestamp)))
            }
            Event::Window { win_event: sdl2::event::WindowEvent::Resized(w, h), timestamp, .. } => {
                let draw_size = self.draw_size();
                return Some(input::Event::Input(Input::Resize(ResizeArgs {
                    window_size: [w as f64, h as f64],
                    draw_size: draw_size.into(),
                }), Some(timestamp)));
            }
            Event::Window { win_event: WindowEvent::FocusGained, timestamp, .. } => {
                return Some(input::Event::Input(Input::Focus(true), Some(timestamp)));
            }
            Event::Window { win_event: WindowEvent::FocusLost, timestamp, .. } => {
                return Some(input::Event::Input(Input::Focus(false), Some(timestamp)));
            }
            Event::Window { win_event: WindowEvent::Enter, timestamp, .. } => {
                return Some(input::Event::Input(Input::Cursor(true), Some(timestamp)));
            }
            Event::Window { win_event: WindowEvent::Leave, timestamp, .. } => {
                return Some(input::Event::Input(Input::Cursor(false), Some(timestamp)));
            }
            _ => {
                *unknown = true;
                return None;
            }
        }
        None
    }

    fn fake_capture(&mut self) {
        // Fake capturing of cursor.
        let (w, h) = self.window.size();
        let cx = (w / 2) as i32;
        let cy = (h / 2) as i32;
        let s = self.sdl_context.event_pump().unwrap().mouse_state();
        let dx = cx - s.x();
        let dy = cy - s.y();
        if dx != 0 || dy != 0 {
            self.ignore_relative_event = Some((dx, dy));
            self.sdl_context.mouse().warp_mouse_in_window(&self.window, cx as i32, cy as i32);
        }
    }
}

impl BuildFromWindowSettings for Sdl2Window {
    fn build_from_window_settings(settings: &WindowSettings) -> Result<Self, Box<dyn Error>> {
        Sdl2Window::new(settings)
    }
}

impl Drop for Sdl2Window {
    fn drop(&mut self) {
        self.set_capture_cursor(false);
    }
}

impl Window for Sdl2Window {
    fn should_close(&self) -> bool {
        self.should_close
    }
    fn set_should_close(&mut self, value: bool) {
        self.should_close = value;
    }
    fn swap_buffers(&mut self) {
        self.window.gl_swap_window();
    }
    fn size(&self) -> Size {
        let (w, h) = self.window.size();
        Size {width: w as f64, height: h as f64}
    }
    fn wait_event(&mut self) -> Event {
        self.wait_event()
    }
    fn wait_event_timeout(&mut self, timeout: Duration) -> Option<Event> {
        self.wait_event_timeout(timeout)
    }
    fn poll_event(&mut self) -> Option<Event> {
        self.poll_event()
    }
    fn draw_size(&self) -> Size {
        let (w, h) = self.window.drawable_size();
        Size {width: w as f64, height: h as f64}
    }
}

impl AdvancedWindow for Sdl2Window {
    fn get_title(&self) -> String {
        self.title.clone()
    }
    fn set_title(&mut self, value: String) {
        let _ = self.window.set_title(&value);
        self.title = value
    }
    fn get_automatic_close(&self) -> bool {
        self.automatic_close
    }
    fn set_automatic_close(&mut self, value: bool) {
        self.automatic_close = value;
    }
    fn get_exit_on_esc(&self) -> bool {
        self.exit_on_esc
    }
    fn set_exit_on_esc(&mut self, value: bool) {
        self.exit_on_esc = value;
    }
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
    fn show(&mut self) {
        self.window.show();
    }
    fn hide(&mut self) {
        self.window.hide();
    }
    fn get_position(&self) -> Option<Position> {
        let (x, y) = self.window.position();
        Some(Position { x: x, y: y })
    }
    fn set_position<P: Into<Position>>(&mut self, pos: P) {
        use sdl2::video::WindowPos;

        let pos: Position = pos.into();
        self.window.set_position(WindowPos::Positioned(pos.x), WindowPos::Positioned(pos.y));
    }
    fn set_size<S: Into<Size>>(&mut self, size: S) {
        let size: Size = size.into();
        let _ = self.window.set_size(size.width as u32, size.height as u32);
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
