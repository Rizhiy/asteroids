mod color;
mod framebuffer;
mod objects;
mod world;

use color::Color;
use framebuffer::FrameBuffer;
use glam::{Vec2, vec2};
use objects::Asteroid;
use pixels::{Pixels, SurfaceTexture};
use std::pin::Pin;
use std::time::Instant;
use std::{collections::HashSet, time::Duration};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use world::WorldState;

const CAMERA_SPEED: f32 = 300.0;
const RANDOM_SPAWN_RATE_INITIAL: f32 = 10.0;
const RANDOM_SPAWN_RATE_INCREASE: f32 = 2.0;
const STATS_UPDATE_RATE: f32 = 5.0;
const FPS_TARGET: f32 = 60.0;
const SPEED_ADJUST_FACTOR: f32 = 1.5;

fn power_law_sample(min_value: f32, alpha: f32) -> f32 {
    let u = fastrand::f32();
    min_value * (1.0 - u).powf(-1.0 / alpha)
}

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

#[derive(Default)]
struct InputState {
    camera_pos: Vec2,
    camera_vel: Vec2,
    cursor_pos: Vec2,
    creating_asteroid: bool,
    asteroid_start_pos: Vec2,
    asteroid_size: f32,
    asteroid_hold_time: f32,
    keys_pressed: HashSet<KeyCode>,
    mouse_buttons_pressed: HashSet<MouseButton>,
    pub speed_multiplier: f32,
    previous_speed: f32,
    camera_tracking: bool,
    pub zoom: f32,
}

impl InputState {
    fn new() -> Self {
        Self {
            asteroid_size: 1.0,
            speed_multiplier: 1.0,
            previous_speed: 1.0,
            zoom: 1.0,
            ..Default::default()
        }
    }

    fn adjust_speed(&mut self, factor: f32) {
        self.speed_multiplier *= factor;
        self.speed_multiplier = self.speed_multiplier.clamp(0.1, 100.0);
    }

    fn reset_speed(&mut self) {
        self.speed_multiplier = 1.0;
    }

    fn toggle_pause(&mut self) {
        if self.speed_multiplier == 0.0 {
            self.speed_multiplier = self.previous_speed;
        } else {
            self.previous_speed = self.speed_multiplier;
            self.speed_multiplier = 0.0;
        }
    }

    fn screen_to_world(&self, screen_pos: Vec2, window_size: (u32, u32)) -> Vec2 {
        let screen_center = vec2(window_size.0 as f32 / 2.0, window_size.1 as f32 / 2.0);
        (screen_pos - screen_center) / self.zoom + self.camera_pos
    }

    fn apply_zoom(&mut self, cursor_pos: Vec2, window_size: (u32, u32), zoom_factor: f32) {
        // Calculate world position under cursor before zoom
        let world_pos_before = self.screen_to_world(cursor_pos, window_size);

        // Apply zoom
        self.zoom *= zoom_factor;
        self.zoom = self.zoom.clamp(0.01, 10.0);

        // Calculate where cursor would be in world coords with new zoom if camera stayed same
        let world_pos_after = self.screen_to_world(cursor_pos, window_size);

        // Adjust camera to keep world position under cursor constant
        self.camera_pos += world_pos_before - world_pos_after;
    }

    fn reset_zoom(&mut self) {
        self.zoom = 1.0;
    }

    fn start_creating_asteroid(&mut self, screen_pos: Vec2, window_size: (u32, u32)) {
        self.creating_asteroid = true;
        let screen_center = vec2(window_size.0 as f32 / 2.0, window_size.1 as f32 / 2.0);
        self.asteroid_start_pos = (screen_pos - screen_center) / self.zoom + self.camera_pos;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;
    }

    fn update_asteroid_size(&mut self, dt: f32) {
        if self.creating_asteroid && self.mouse_buttons_pressed.contains(&MouseButton::Left) {
            self.asteroid_hold_time += dt;
            let scaled_time = self.asteroid_hold_time * 10.0;
            self.asteroid_size = 1.0 + scaled_time * scaled_time;
        }
    }

    fn finish_creating_asteroid(
        &mut self,
        screen_pos: Vec2,
        window_size: (u32, u32),
    ) -> (Vec2, Vec2, f32) {
        let screen_center = vec2(window_size.0 as f32 / 2.0, window_size.1 as f32 / 2.0);
        let world_end_pos = (screen_pos - screen_center) / self.zoom + self.camera_pos;

        let pos = self.asteroid_start_pos;
        let vel = (world_end_pos - self.asteroid_start_pos) + self.camera_vel;
        let size = self.asteroid_size;

        self.creating_asteroid = false;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;

        (pos, vel, size)
    }
}

impl InputState {
    fn update_camera(&mut self, dt: f32) {
        self.camera_vel = vec2(0.0, 0.0);

        let speed = CAMERA_SPEED / self.zoom;

        if self.keys_pressed.contains(&KeyCode::KeyW) {
            self.camera_vel.y -= speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            self.camera_vel.y += speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            self.camera_vel.x -= speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            self.camera_vel.x += speed;
        }

        self.camera_pos = self.camera_pos + self.camera_vel * dt;
    }
}

struct RunningState {
    framebuffer: FrameBuffer,
    window: Pin<Box<Window>>,
    world: WorldState,
    input: InputState,
    last_frame_time: Instant,
    frame_count: u32,
    last_fps_time: Instant,
    frames_per_second: f32,
    stats_changed: bool,
    last_update_time: Instant,
    random_spawn_timer: f32,
    random_spawn_hold_time: f32,
    window_visible: bool,
}

impl RunningState {
    fn new(window: Window) -> Self {
        let window = Box::pin(window);
        let w_ref: &Window = window.as_ref().get_ref();

        let size = w_ref.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, w_ref);

        let pixels = pixels::PixelsBuilder::new(size.width, size.height, surface)
            .present_mode(pixels::wgpu::PresentMode::Mailbox)
            .build()
            .unwrap();

        let pixels: Pixels<'static> =
            unsafe { std::mem::transmute::<Pixels<'_>, Pixels<'static>>(pixels) };

        let framebuffer = FrameBuffer::new(pixels, size.width, size.height);

        let now = Instant::now();
        Self {
            framebuffer,
            window,
            world: WorldState::new(),
            input: InputState::new(),
            last_frame_time: now,
            frame_count: 0,
            last_fps_time: now,
            frames_per_second: 0.0,
            last_update_time: now,
            random_spawn_timer: 0.0,
            random_spawn_hold_time: 0.0,
            window_visible: true,
            stats_changed: false,
        }
    }

    fn window(&self) -> &Window {
        self.window.as_ref().get_ref()
    }

    fn window_mut(&mut self) -> &Window {
        self.window.as_ref().get_ref()
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.framebuffer.resize(width, height).unwrap();
    }
}

impl RunningState {
    fn draw(&mut self) {
        self.framebuffer.set_camera_pos(self.input.camera_pos);
        self.framebuffer.set_zoom(self.input.zoom);
        self.framebuffer.clear(Color::BLACK);

        for asteroid in &self.world.asteroids {
            asteroid.draw(&mut self.framebuffer, Color::WHITE);
        }

        if self.input.creating_asteroid {
            let preview = Asteroid::new(
                self.input.asteroid_start_pos,
                vec2(0.0, 0.0),
                self.input.asteroid_size,
            );
            preview.draw(&mut self.framebuffer, Color::WHITE);
        }

        let stats_text = format!(
            "FPS: {} | UPS: {} | Asteroids: {}",
            self.frames_per_second as u32,
            self.world.updates_per_second() as u32,
            self.world.asteroids.len()
        );
        let text_pos = vec2(10.0, 10.0);
        self.framebuffer
            .draw_text(&stats_text, text_pos, 16.0, Color::WHITE);

        let speed_text = format!(
            "Speed: {:.1}x | Zoom: {:.2}x",
            self.input.speed_multiplier, self.input.zoom
        );
        let window_size = self.window().inner_size();
        let text_width = speed_text.len() as f32 * 10.0;
        let speed_pos = vec2(window_size.width as f32 - text_width - 10.0, 10.0);
        self.framebuffer
            .draw_text(&speed_text, speed_pos, 16.0, Color::WHITE);

        self.framebuffer.render().unwrap();

        self.frame_count += 1;
        let update_interval = 1.0 / STATS_UPDATE_RATE;
        if self.last_fps_time.elapsed().as_secs_f32() >= update_interval {
            self.frames_per_second =
                self.frame_count as f32 / self.last_fps_time.elapsed().as_secs_f32();
            self.frame_count = 0;
            self.last_fps_time = Instant::now();
            self.stats_changed = true;
        }
    }

    fn on_press(&mut self) {
        if !self.input.creating_asteroid {
            let screen_pos = self.input.cursor_pos;
            let window_size = self.window().inner_size();
            self.input
                .start_creating_asteroid(screen_pos, (window_size.width, window_size.height));
        } else if !self
            .input
            .mouse_buttons_pressed
            .contains(&MouseButton::Left)
        {
            let screen_pos = self.input.cursor_pos;
            let window_size = self.window().inner_size();
            let (pos, vel, size) = self
                .input
                .finish_creating_asteroid(screen_pos, (window_size.width, window_size.height));
            self.world.spawn_asteroid(pos, vel, size);
        }
    }

    fn spawn_random_asteroid(&mut self) {
        let window_size = self.window().inner_size();
        let width = window_size.width as f32 / self.input.zoom;
        let height = window_size.height as f32 / self.input.zoom;

        let x = self.input.camera_pos.x + (fastrand::f32() - 0.5) * width;
        let y = self.input.camera_pos.y + (fastrand::f32() - 0.5) * height;
        let pos = vec2(x, y);

        let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;

        // Power law distribution for speed: alpha=1.354 gives ~1% with speed > 30
        let speed = power_law_sample(1.0, 1.354).min(100.0);
        let random_vel = vec2(angle.cos() * speed, angle.sin() * speed);
        let vel = random_vel + self.input.camera_vel;

        // Power law distribution for size: alpha=0.667 gives ~1% with size > 1000
        let size = power_law_sample(1.0, 0.667).min(10000.0);

        self.world.spawn_asteroid(pos, vel, size);
    }

    fn update(&mut self) {
        let mut dt = Instant::now()
            .duration_since(self.last_frame_time)
            .as_secs_f32();

        let target_frame_time = 1.0 / FPS_TARGET;
        if dt < target_frame_time {
            let sleep_duration = std::time::Duration::from_secs_f32(target_frame_time - dt);
            std::thread::sleep(sleep_duration);
            dt = Instant::now()
                .duration_since(self.last_frame_time)
                .as_secs_f32();
        }
        self.last_frame_time = Instant::now();
        let elapsed = self.last_update_time.elapsed();
        let update_start = Instant::now();

        if !self.input.camera_tracking {
            self.input.update_camera(dt);
        } else {
            self.input.camera_vel = vec2(0.0, 0.0);
        }

        let mut update_secs = 0.0;

        if self.input.speed_multiplier != 0.0 {
            // Calculate how much we should update the simulation by
            let scaled_time = elapsed.as_secs_f32() * self.input.speed_multiplier;
            let scaled_update_time = self.world.update(scaled_time);
            update_secs = scaled_update_time / self.input.speed_multiplier;
        }
        let update_time = Duration::from_secs_f32(update_secs);

        if self.input.camera_tracking {
            let center = self.world.calculate_center_of_mass(true);
            let new_camera_pos = center;

            self.input.camera_vel = (new_camera_pos - self.input.camera_pos) / dt;
            self.input.camera_pos = new_camera_pos;
        }

        self.input.update_asteroid_size(dt);

        if self.input.keys_pressed.contains(&KeyCode::KeyR) {
            self.random_spawn_timer += dt;
            self.random_spawn_hold_time += dt;

            let current_spawn_rate = RANDOM_SPAWN_RATE_INITIAL
                + (self.random_spawn_hold_time * RANDOM_SPAWN_RATE_INCREASE);

            let spawn_interval = 1.0 / current_spawn_rate;
            while self.random_spawn_timer >= spawn_interval {
                self.spawn_random_asteroid();
                self.random_spawn_timer -= spawn_interval;
            }
        } else {
            self.random_spawn_timer = 0.0;
            self.random_spawn_hold_time = 0.0;
        }

        // If it takes longer to simulate certain amount of world time than how much we need to simulate, the simulation will always try to catch up
        // This prevents spiraling when the simulation is too heavy
        let update_duration = update_start.elapsed();
        if update_time > Duration::ZERO && update_duration > update_time {
            self.last_update_time = Instant::now();
        } else {
            self.last_update_time += update_time;
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        let AppState::Running(running) = &mut self.state else {
            return;
        };

        running.update();

        if running.window_visible {
            running.window().request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::Running(_)) {
            return;
        }

        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .expect("No monitor found");

        let screen_size = monitor.size();
        let width = screen_size.width / 2;
        let height = screen_size.height / 2;

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Asteroids")
                    .with_inner_size(PhysicalSize::new(width, height)),
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
                running.input.cursor_pos = vec2(position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    if button == MouseButton::Left {
                        running.on_press();
                    }
                    running.input.mouse_buttons_pressed.insert(button);
                }
                ElementState::Released => {
                    running.input.mouse_buttons_pressed.remove(&button);
                }
            },

            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_x, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                if delta_y != 0.0 {
                    let window_size = running.window().inner_size();
                    let cursor_pos = running.input.cursor_pos;
                    let zoom_factor = if delta_y > 0.0 { 1.2 } else { 1.0 / 1.2 };

                    running.input.apply_zoom(
                        cursor_pos,
                        (window_size.width, window_size.height),
                        zoom_factor,
                    );
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            running.input.keys_pressed.insert(keycode);

                            // Handle T key to start tracking
                            if keycode == KeyCode::KeyT {
                                running.input.camera_tracking = true;
                            }

                            if keycode == KeyCode::KeyP {
                                running.input.toggle_pause();
                                running.stats_changed = true;
                            }

                            if keycode == KeyCode::KeyZ {
                                running.input.reset_zoom();
                            }

                            let shift_pressed =
                                running.input.keys_pressed.contains(&KeyCode::ShiftLeft)
                                    || running.input.keys_pressed.contains(&KeyCode::ShiftRight);

                            if (keycode == KeyCode::Equal && shift_pressed)
                                || keycode == KeyCode::NumpadAdd
                            {
                                running.input.adjust_speed(SPEED_ADJUST_FACTOR);
                                running.stats_changed = true;
                            } else if keycode == KeyCode::Equal {
                                running.input.reset_speed();
                                running.stats_changed = true;
                            }
                            if keycode == KeyCode::Minus || keycode == KeyCode::NumpadSubtract {
                                running.input.adjust_speed(1.0 / SPEED_ADJUST_FACTOR);
                                running.stats_changed = true;
                            }

                            if matches!(
                                keycode,
                                KeyCode::KeyW | KeyCode::KeyS | KeyCode::KeyA | KeyCode::KeyD
                            ) {
                                running.input.camera_tracking = false;
                            }
                        }
                        ElementState::Released => {
                            running.input.keys_pressed.remove(&keycode);
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Only draw and present when window is visible
                // This prevents blocking on present() when window is hidden on Wayland
                if running.window_visible {
                    running.draw();
                }
            }

            WindowEvent::Occluded(occluded) => {
                running.window_visible = !occluded;
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
