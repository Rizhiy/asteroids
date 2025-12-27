mod color;
mod framebuffer;
mod objects;
mod vector;

use color::Color;
use framebuffer::FrameBuffer;
use objects::Asteroid;
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

struct WorldState {
    asteroids: Vec<Asteroid>,
    last_update_time: std::time::Instant,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            asteroids: Vec::new(),
            last_update_time: std::time::Instant::now(),
        }
    }
}

impl WorldState {
    fn new() -> Self {
        Default::default()
    }

    fn update(&mut self) {
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
        }
    }

    fn check_collisions(&mut self) {
        use std::collections::HashSet;

        let mut to_remove = HashSet::new();
        let mut to_add = Vec::new();

        // Check all pairs (i, j) where i < j
        for i in 0..self.asteroids.len() {
            for j in (i + 1)..self.asteroids.len() {
                // Skip if already marked for removal
                if to_remove.contains(&i) || to_remove.contains(&j) {
                    continue;
                }

                let a1 = &self.asteroids[i];
                let a2 = &self.asteroids[j];

                // Check collision
                if a1.collides_with(a2) {
                    // Merge them
                    let merged = a1.merge_with(a2);
                    to_add.push(merged);

                    // Mark both for removal
                    to_remove.insert(i);
                    to_remove.insert(j);
                }
            }
        }

        // Remove using retain with index
        let mut idx = 0;
        self.asteroids.retain(|_| {
            let should_keep = !to_remove.contains(&idx);
            idx += 1;
            should_keep
        });

        self.asteroids.extend(to_add);
    }

    fn spawn_asteroid(&mut self, pos: Vector, vel: Vector, size: f32) {
        self.asteroids.push(Asteroid::new(pos, vel, size));
    }
}

#[derive(Default)]
struct InputState {
    camera_pos: Vector,

    cursor_pos: Vector,
    button_pressed: bool,
    asteroid_size: f32,

    key_w_pressed: bool,
    key_s_pressed: bool,
    key_a_pressed: bool,
    key_d_pressed: bool,
}

impl InputState {
    fn new() -> Self {
        Self {
            asteroid_size: 1.0,
            ..Default::default()
        }
    }
}

impl InputState {
    fn update_camera(&mut self) {
        if self.key_w_pressed {
            self.camera_pos.y -= CAMERA_SPEED;
        }
        if self.key_s_pressed {
            self.camera_pos.y += CAMERA_SPEED;
        }
        if self.key_a_pressed {
            self.camera_pos.x -= CAMERA_SPEED;
        }
        if self.key_d_pressed {
            self.camera_pos.x += CAMERA_SPEED;
        }
    }

    fn world_pos_from_cursor(&self) -> Vector {
        self.camera_pos + self.cursor_pos
    }
}

/// Window and rendering state
///
/// IMPORTANT invariants:
/// - `window` is pinned, so its address won't change.
/// - `framebuffer` must be dropped before `window` (field order does that).
struct RunningState {
    // Put framebuffer FIRST so it is dropped BEFORE window.
    framebuffer: FrameBuffer,
    window: Pin<Box<Window>>,

    // Game state
    world: WorldState,
    input: InputState,
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

        let framebuffer = FrameBuffer::new(pixels, WIDTH, HEIGHT);

        Self {
            framebuffer,
            window,
            world: WorldState::new(),
            input: InputState::new(),
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
        self.framebuffer.resize(width, height).unwrap();
    }

    fn draw(&mut self) {
        self.framebuffer.set_camera_pos(self.input.camera_pos);
        self.framebuffer.clear(Color::BLACK);

        // Draw all world asteroids
        for asteroid in &self.world.asteroids {
            asteroid.draw(&mut self.framebuffer, Color::WHITE);
        }

        // Draw preview asteroid being created
        if self.input.button_pressed {
            let world_pos = self.input.world_pos_from_cursor();
            let preview = Asteroid::new(
                world_pos,
                Vector { x: 0.0, y: 0.0 },
                self.input.asteroid_size,
            );
            preview.draw(&mut self.framebuffer, Color::WHITE);
        }

        self.framebuffer.render().unwrap();
    }

    fn on_release(&mut self) {
        let world_pos = self.input.world_pos_from_cursor();
        self.world.spawn_asteroid(
            world_pos,
            Vector { x: 0.0, y: 0.0 },
            self.input.asteroid_size,
        );
        self.input.asteroid_size = 1.0;
    }

    fn update(&mut self) {
        // Update camera position every frame
        self.input.update_camera();

        // Update world physics
        self.world.update();

        // Update asteroid size while button held
        if self.input.button_pressed {
            self.input.asteroid_size += 1.0;
        }
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
                running.input.cursor_pos = Vector {
                    x: position.x as f32,
                    y: position.y as f32,
                };
            }

            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => {
                    running.input.button_pressed = true;
                }
                (ElementState::Released, MouseButton::Left) => {
                    running.on_release();
                    running.input.button_pressed = false;
                }

                _ => {}
            },

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    let is_pressed = event.state == ElementState::Pressed;
                    match keycode {
                        KeyCode::KeyW => running.input.key_w_pressed = is_pressed,
                        KeyCode::KeyS => running.input.key_s_pressed = is_pressed,
                        KeyCode::KeyA => running.input.key_a_pressed = is_pressed,
                        KeyCode::KeyD => running.input.key_d_pressed = is_pressed,
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
            1.0 / running.world.last_update_time.elapsed().as_secs_f32()
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
