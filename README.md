# sdl2_game_window [![Build Status](https://travis-ci.org/PistonDevelopers/sdl2_game_window.svg)](https://travis-ci.org/PistonDevelopers/sdl2_game_window)

An SDL2 back-end for the Piston game engine

Maintainers: @TyOverby, @bvssvni, @Coeuvre

### How to create a window

```Rust
let mut window = WindowSDL2::new(
    piston::shader_version::opengl::OpenGL_3_2,
    piston::WindowSettings {
        title: "My application".to_string(),
        size: [640, 480],
        fullscreen: false,
        exit_on_esc: true,
        samples: 4,
    }
);
```

### How to set up Gfx

After you have created a window, do the following:

```Rust
let mut device = gfx::GlDevice::new(|s| unsafe {
    transmute(sdl2::video::gl_get_proc_address(s))
});
let (w, h) = window.get_size();
let frame = gfx::Frame::new(w as u16, h as u16);
```

### Troubleshooting

* [I get `ld: library not found for -lSDL2` error on OSX](https://github.com/PistonDevelopers/rust-empty/issues/175)
