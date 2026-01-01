mod color;
mod framebuffer;
mod objects;
mod spawn_strategy;
mod world;

use color::Color;
use framebuffer::FrameBuffer;
use glam::vec2;
use objects::Asteroid;
use pixels::{Pixels, SurfaceTexture};
use spawn_strategy::{OrbitalDiskStrategy, RandomScreenSpaceStrategy, SpawnStrategy};
use std::pin::Pin;
use std::time::Duration;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use world::WorldState;
const RANDOM_SPAWN_RATE_INITIAL: f32 = 10.0;
const RANDOM_SPAWN_RATE_INCREASE: f32 = 2.0;
const STATS_UPDATE_RATE: f32 = 5.0;
const FPS_TARGET: f32 = 60.0;
const SPEED_ADJUST_FACTOR: f32 = 1.5;
const MAX_SPEED_MULTIPLIER_RATIO: f32 = 5.0;

fn format_time(seconds: f32) -> String {
    let total_seconds = seconds as i64;
    let days = total_seconds / (24 * 3600);
    let remainder = total_seconds % (24 * 3600);
    let hours = remainder / 3600;
    let remainder = remainder % 3600;
    let minutes = remainder / 60;
    let secs = remainder % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
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

struct RunningState {
    framebuffer: FrameBuffer,
    window: Pin<Box<Window>>,
    world: WorldState,
    last_frame_time: Instant,
    frame_count: u32,
    last_fps_time: Instant,
    frames_per_second: f32,
    stats_changed: bool,
    last_update_time: Instant,
    random_spawn_timer: f32,
    random_spawn_hold_time: f32,
    window_visible: bool,
    spawn_strategy: Box<dyn SpawnStrategy>,
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
            last_frame_time: now,
            frame_count: 0,
            last_fps_time: now,
            frames_per_second: 0.0,
            last_update_time: now,
            random_spawn_timer: 0.0,
            random_spawn_hold_time: 0.0,
            window_visible: true,
            stats_changed: false,
            spawn_strategy: Box::new(OrbitalDiskStrategy::new()),
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
        let mut clear_color = Color::BLACK;
        clear_color.a = 0;
        self.framebuffer.clear(clear_color);

        for asteroid in &self.world.asteroids {
            asteroid.draw(&mut self.framebuffer, Color::WHITE);
        }

        if self.framebuffer.creating_asteroid {
            let preview = Asteroid::new(
                self.framebuffer.asteroid_start_pos,
                vec2(0.0, 0.0),
                self.framebuffer.asteroid_size,
            );
            preview.draw(&mut self.framebuffer, Color::WHITE);
        }

        let stats_text = format!(
            "FPS: {} | UPS: {}",
            self.frames_per_second as u32,
            self.world.updates_per_second() as u32
        );
        let text_pos = vec2(10.0, 10.0);
        self.framebuffer
            .draw_text(&stats_text, text_pos, 16.0, Color::WHITE);

        let time_text = format!(
            "Time: {} | Asteroids: {}",
            format_time(self.world.world_time),
            self.world.asteroids.len()
        );
        let time_pos = vec2(10.0, 30.0);
        self.framebuffer
            .draw_text(&time_text, time_pos, 16.0, Color::WHITE);

        let speed_text = format!(
            "Speed: {:.1}x | Zoom: {:.2}x | Spawn: {}",
            self.framebuffer.speed_multiplier,
            self.framebuffer.zoom,
            self.spawn_strategy.name()
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
        if !self.framebuffer.creating_asteroid {
            let screen_pos = self.framebuffer.cursor_pos;
            self.framebuffer.start_creating_asteroid(screen_pos);
        } else if !self
            .framebuffer
            .mouse_buttons_pressed
            .contains(&MouseButton::Left)
        {
            let screen_pos = self.framebuffer.cursor_pos;
            // Scale velocity based on actual simulation speed (UPS / tick_rate)
            let actual_speed = self.world.updates_per_second() / self.world.tick_rate();

            let (pos, vel, size) = self
                .framebuffer
                .finish_creating_asteroid(screen_pos, actual_speed);
            self.world.spawn_asteroid(pos, vel, size);
        }
    }

    fn spawn_asteroids(&mut self) {
        let asteroids = self.spawn_strategy.spawn(&self.world, &self.framebuffer);

        for asteroid in asteroids {
            self.world.asteroids.push(asteroid);
        }
    }

    fn toggle_spawn_strategy(&mut self) {
        let current_name = self.spawn_strategy.name();
        self.spawn_strategy = match current_name {
            "Random" => Box::new(OrbitalDiskStrategy::new()),
            "Orbital" => Box::new(RandomScreenSpaceStrategy::new()),
            _ => Box::new(RandomScreenSpaceStrategy::new()),
        };
        self.stats_changed = true;
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

        if !self.framebuffer.camera_tracking {
            self.framebuffer.update_camera(dt);
        } else {
            self.framebuffer.camera_vel = vec2(0.0, 0.0);
        }

        let mut update_secs = elapsed.as_secs_f32();

        if self.framebuffer.speed_multiplier != 0.0 {
            // Clamp speed multiplier to prevent simulation from falling behind and reducing FPS
            let actual_speed = self.world.updates_per_second() / self.world.tick_rate();
            let max_allowed_speed = (actual_speed * MAX_SPEED_MULTIPLIER_RATIO).max(0.01);
            let effective_speed = self.framebuffer.speed_multiplier.min(max_allowed_speed);

            // Calculate how much we should update the simulation by
            let scaled_time = elapsed.as_secs_f32() * effective_speed;
            let scaled_update_time = self.world.update(scaled_time);
            update_secs = scaled_update_time / effective_speed.max(0.01);
        }
        let update_time = Duration::from_secs_f32(update_secs);

        if self.framebuffer.camera_tracking {
            let center = self.world.calculate_center_of_mass(true);
            let new_camera_pos = center;

            self.framebuffer.camera_vel = (new_camera_pos - self.framebuffer.camera_pos) / dt;
            self.framebuffer.camera_pos = new_camera_pos;
        }

        self.framebuffer.update_asteroid_size(dt);

        if self.framebuffer.keys_pressed.contains(&KeyCode::KeyR) {
            self.random_spawn_timer += dt;
            self.random_spawn_hold_time += dt;

            let current_spawn_rate = RANDOM_SPAWN_RATE_INITIAL
                + (self.random_spawn_hold_time * RANDOM_SPAWN_RATE_INCREASE);

            let spawn_interval = 1.0 / current_spawn_rate;
            while self.random_spawn_timer >= spawn_interval {
                self.spawn_asteroids();
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
                running.framebuffer.cursor_pos = vec2(position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    if button == MouseButton::Left {
                        running.on_press();
                    }
                    running.framebuffer.mouse_buttons_pressed.insert(button);
                }
                ElementState::Released => {
                    running.framebuffer.mouse_buttons_pressed.remove(&button);
                }
            },

            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_x, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };

                if delta_y != 0.0 {
                    let cursor_pos = running.framebuffer.cursor_pos;
                    let zoom_factor = if delta_y > 0.0 { 1.2 } else { 1.0 / 1.2 };

                    running.framebuffer.apply_zoom(cursor_pos, zoom_factor);
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            running.framebuffer.keys_pressed.insert(keycode);

                            // Handle T key to start tracking
                            if keycode == KeyCode::KeyT {
                                running.framebuffer.camera_tracking = true;
                            }

                            if keycode == KeyCode::KeyP {
                                running.framebuffer.toggle_pause();
                                running.stats_changed = true;
                            }

                            if keycode == KeyCode::KeyZ {
                                running.framebuffer.reset_zoom();
                            }

                            if keycode == KeyCode::KeyO {
                                running.toggle_spawn_strategy();
                            }

                            let shift_pressed = running
                                .framebuffer
                                .keys_pressed
                                .contains(&KeyCode::ShiftLeft)
                                || running
                                    .framebuffer
                                    .keys_pressed
                                    .contains(&KeyCode::ShiftRight);

                            if (keycode == KeyCode::Equal && shift_pressed)
                                || keycode == KeyCode::NumpadAdd
                            {
                                running.framebuffer.adjust_speed(SPEED_ADJUST_FACTOR);
                                running.stats_changed = true;
                            } else if keycode == KeyCode::Equal {
                                running.framebuffer.reset_speed();
                                running.stats_changed = true;
                            }
                            if keycode == KeyCode::Minus || keycode == KeyCode::NumpadSubtract {
                                running.framebuffer.adjust_speed(1.0 / SPEED_ADJUST_FACTOR);
                                running.stats_changed = true;
                            }

                            if matches!(
                                keycode,
                                KeyCode::KeyW | KeyCode::KeyS | KeyCode::KeyA | KeyCode::KeyD
                            ) {
                                running.framebuffer.camera_tracking = false;
                            }
                        }
                        ElementState::Released => {
                            running.framebuffer.keys_pressed.remove(&keycode);
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
