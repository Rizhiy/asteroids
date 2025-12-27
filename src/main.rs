mod objects;
mod vector;

use objects::{Asteroid, Color};
use pixels::{Pixels, SurfaceTexture};
use std::io::{self, Write};
use std::pin::Pin;
use vector::Vector;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const WIDTH: u32 = 1920 / 2;
const HEIGHT: u32 = 1080 / 2;
const TICK_RATE: f32 = 100.0;
const CAMERA_SPEED: f32 = 5.0; // pixels per frame

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
    last_update_time: std::time::Instant,
    // Spawning asteroids
    button_pressed: bool,
    asteroid_size: f32,
    // Camera movement
    key_w_pressed: bool,
    key_s_pressed: bool,
    key_a_pressed: bool,
    key_d_pressed: bool,
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

        Self {
            pixels,
            window,
            cursor_pos: Vector { x: 0.0, y: 0.0 },
            window_pos: Vector { x: 0.0, y: 0.0 },
            asteroids: Vec::<Asteroid>::new(),
            last_update_time: std::time::Instant::now(),
            button_pressed: false,
            asteroid_size: 1.0,
            key_w_pressed: false,
            key_s_pressed: false,
            key_a_pressed: false,
            key_d_pressed: false,
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

    fn set_pixel(frame: &mut [u8], world_pos: Vector, window_pos: Vector, color: Color) {
        // Convert world coordinates to screen coordinates
        let screen_pos = world_pos - window_pos;
        
        if screen_pos.x < 0.0 || screen_pos.x >= WIDTH as f32 || screen_pos.y < 0.0 || screen_pos.y >= HEIGHT as f32 {
            return;
        }
        let index = ((screen_pos.y as u32 * WIDTH + screen_pos.x as u32) * 4) as usize;
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
        let window_pos = self.window_pos;
        let frame = self.pixels.frame_mut();
        Self::clear_frame(frame, Color::BLACK);

        for asteroid in &self.asteroids {
            // TODO: Move this logic inside asteroid
            let ceil_radius = asteroid.radius().ceil() as i32;
            let true_radius = asteroid.radius();
            for x_offset in -ceil_radius..ceil_radius {
                for y_offset in -ceil_radius..ceil_radius {
                    let pixel_pos = asteroid.pos()
                        + vector::Vector {
                            x: x_offset as f32,
                            y: y_offset as f32,
                        };
                    if (pixel_pos - asteroid.pos()).length() <= true_radius {
                        Self::set_pixel(frame, pixel_pos, window_pos, Color::WHITE);
                    }
                }
            }
        }

        self.pixels.render().unwrap();
    }

    fn on_release(&mut self) {
        let asteroid = Asteroid::new(
            self.window_pos + self.cursor_pos,
            vector::Vector { x: 0.0, y: 0.0 },
            self.asteroid_size,
        );
        self.asteroids.push(asteroid);
        self.asteroid_size = 1.0;
    }

    fn update_camera(&mut self) {
        if self.key_w_pressed {
            self.window_pos.y -= CAMERA_SPEED;
        }
        if self.key_s_pressed {
            self.window_pos.y += CAMERA_SPEED;
        }
        if self.key_a_pressed {
            self.window_pos.x -= CAMERA_SPEED;
        }
        if self.key_d_pressed {
            self.window_pos.x += CAMERA_SPEED;
        }
    }

    fn update(&mut self) {
        // Update camera position every frame
        self.update_camera();

        let mut delta = self.last_update_time.elapsed();
        let tick_duration = std::time::Duration::from_secs_f32(1.0 / TICK_RATE);

        while delta >= tick_duration {
            let new_asteroids: Vec<Asteroid> = self
                .asteroids
                .iter()
                .map(|asteroid| {
                    let mut a = *asteroid;
                    a.update(&self.asteroids, 1.0 / TICK_RATE);
                    a
                })
                .collect();
            self.asteroids = new_asteroids;

            // Check for collisions and merge asteroids
            self.check_collisions();

            delta -= tick_duration;
            self.last_update_time += tick_duration;

            if self.button_pressed {
                self.asteroid_size += 1.0;
            }
        }
    }

    fn check_collisions(&mut self) {
        let mut to_remove = Vec::new();
        let mut to_add = Vec::new();

        for i in 0..self.asteroids.len() {
            for j in (i + 1)..self.asteroids.len() {
                if to_remove.contains(&i) || to_remove.contains(&j) {
                    continue;
                }

                let a1 = &self.asteroids[i];
                let a2 = &self.asteroids[j];

                if a1.collides_with(a2) {
                    let merged = a1.merge_with(a2);
                    to_add.push(merged);

                    to_remove.push(i);
                    to_remove.push(j);
                }
            }
        }

        to_remove.sort_unstable();
        to_remove.reverse();
        to_remove.dedup();

        for index in to_remove {
            self.asteroids.swap_remove(index);
        }

        self.asteroids.extend(to_add);
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
                    running.button_pressed = true;
                }
                (ElementState::Released, MouseButton::Left) => {
                    running.on_release();
                    running.button_pressed = false;
                }

                _ => {}
            },

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    let is_pressed = event.state == ElementState::Pressed;
                    match keycode {
                        KeyCode::KeyW => running.key_w_pressed = is_pressed,
                        KeyCode::KeyS => running.key_s_pressed = is_pressed,
                        KeyCode::KeyA => running.key_a_pressed = is_pressed,
                        KeyCode::KeyD => running.key_d_pressed = is_pressed,
                        _ => {}
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                running.draw();
                running.window_mut().request_redraw(); // keep animating for now
            }

            _ => {}
        }

        print!("\r\x1B[2K"); // clear the line
        print!(
            "FPS: {}",
            1.0 / running.last_update_time.elapsed().as_secs_f32()
        );
        io::stdout().flush().unwrap();
        running.update();
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
