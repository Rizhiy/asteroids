mod objects;
mod vector;

use objects::{Asteroid, Color};
use pixels::{Pixels, SurfaceTexture};
use std::pin::Pin;
use vector::Vector;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const WIDTH: u32 = 1920 / 2;
const HEIGHT: u32 = 1080 / 2;

enum AppState {
    Starting,
    Running(RunningState),
}

struct App {
    state: AppState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: AppState::Starting,
        }
    }
}

/// Owns the window and pixels.
///
/// IMPORTANT invariants:
/// - `window` is pinned, so its address won't change.
/// - `pixels` must be dropped before `window` (field order does that).
struct RunningState {
    // Put pixels FIRST so it is dropped BEFORE window.
    pixels: Pixels<'static>,
    window: Pin<Box<Window>>,
    cursor_pos: Vector,
    window_pos: Vector,
    asteroids: Vec<Asteroid>,
}

impl RunningState {
    fn new(window: Window) -> Self {
        // Pin the window on the heap => stable address.
        let window = Box::pin(window);

        // Safe: getting a shared reference to a pinned value is fine.
        let w_ref: &Window = window.as_ref().get_ref();

        let size = w_ref.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, w_ref);

        let pixels = Pixels::new(WIDTH, HEIGHT, surface).unwrap();

        // `pixels` really borrows `w_ref` which is tied to `window`.
        // We store it as `'static` and uphold that "as-if-'static" by
        // keeping `window` pinned and ensuring it outlives `pixels`.
        let pixels: Pixels<'static> = unsafe {
            // This is the only unsafe: extending the lifetime.
            // Sound because:
            // 1) window is pinned (won't move),
            // 2) window lives as long as `self`,
            // 3) pixels is dropped before window (field order).
            std::mem::transmute::<Pixels<'_>, Pixels<'static>>(pixels)
        };

        let mut asteroids: Vec<Asteroid> = Vec::new();

        Self {
            pixels,
            window,
            cursor_pos: Vector { x: 0.0, y: 0.0 },
            window_pos: Vector { x: 0.0, y: 0.0 },
            asteroids,
        }
    }

    fn window(&self) -> &Window {
        self.window.as_ref().get_ref()
    }

    fn window_mut(&mut self) -> &Window {
        // We intentionally do NOT give out `&mut Window` because that could allow
        // moving/replace patterns. `Pixels` only needs &Window.
        self.window.as_ref().get_ref()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.pixels.resize_surface(width, height).unwrap();
    }

    fn set_pixel(frame: &mut [u8], pos: Vector, color: Color) {
        if pos.x < 0.0 || pos.x > WIDTH as f32 || pos.y < 0.0 || pos.y > HEIGHT as f32 {
            return;
        }
        let index = ((pos.y as u32 * WIDTH + pos.x as u32) * 4) as usize;
        frame[index] = color.r;
        frame[index + 1] = color.g;
        frame[index + 2] = color.b;
        frame[index + 3] = color.a;
    }

    fn clear_frame(frame: &mut [u8], color: Color) {
        for px in frame.chunks_exact_mut(4) {
            px[0] = color.r;
            px[1] = color.g;
            px[2] = color.b;
            px[3] = color.a;
        }
    }

    fn draw(&mut self) {
        let frame = self.pixels.frame_mut();
        Self::clear_frame(frame, Color::BLACK);

        for asteroid in &self.asteroids {
            Self::set_pixel(frame, asteroid.pos(), Color::WHITE);
        }

        self.pixels.render().unwrap();
    }

    fn on_click(&mut self) {
        let asteroid = Asteroid::new(
            self.window_pos + self.cursor_pos,
            vector::Vector { x: 0.0, y: 0.0 },
            1.0,
        );
        self.asteroids.push(asteroid);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::Running(_)) {
            return;
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("pixels + winit 0.30 (Pin + unsafe)")
                    .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT)),
            )
            .unwrap();

        let running = RunningState::new(window);
        running.window().request_redraw();

        self.state = AppState::Running(running);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let AppState::Running(running) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                running.resize(size.width, size.height);
                running.window_mut().request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                running.cursor_pos = Vector {
                    x: position.x as f32,
                    y: position.y as f32,
                };
            }

            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => {
                    running.on_click();
                }

                _ => {}
            },

            WindowEvent::RedrawRequested => {
                running.draw();
                running.window_mut().request_redraw(); // keep animating for now
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
