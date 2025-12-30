mod color;
mod framebuffer;
mod objects;
mod vector;

use color::Color;
use framebuffer::FrameBuffer;
use objects::Asteroid;
use pixels::{Pixels, SurfaceTexture};
use std::collections::HashSet;
use std::pin::Pin;
use vector::Vector;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const TICK_RATE: f32 = 100.0;
const CAMERA_SPEED: f32 = 300.0; // pixels per second
const RANDOM_SPAWN_RATE_INITIAL: f32 = 10.0; // initial asteroids per second
const RANDOM_SPAWN_RATE_INCREASE: f32 = 2.0; // increase per second of holding
const CLEANUP_THRESHOLD_MULTIPLIER: f32 = 15.0; // how many standard deviations before cleanup
const STATS_UPDATE_RATE: f32 = 5.0; // times per second to update UPS/FPS stats

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
    update_count: u32,
    last_ups_time: std::time::Instant,
    updates_per_second: f32,
    speed_multiplier: f32,
}

impl Default for WorldState {
    fn default() -> Self {
        let now = std::time::Instant::now();
        Self {
            asteroids: Vec::new(),
            last_update_time: now,
            update_count: 0,
            last_ups_time: now,
            updates_per_second: 0.0,
            speed_multiplier: 1.0,
        }
    }
}

impl WorldState {
    fn new() -> Self {
        Default::default()
    }

    fn update(&mut self) -> bool {
        // Scale elapsed time by speed multiplier
        let elapsed = self.last_update_time.elapsed();
        let mut delta = elapsed.mul_f32(self.speed_multiplier);
        let tick_duration = std::time::Duration::from_secs_f32(1.0 / TICK_RATE);
        let mut stats_changed = false;

        // Track how much simulated time we actually processed
        let initial_delta = delta;

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

            // Clean up asteroids that are too far from center of mass
            self.cleanup_distant_asteroids();

            delta -= tick_duration;

            // Track updates per second
            self.update_count += 1;
            let update_interval = 1.0 / STATS_UPDATE_RATE;
            if self.last_ups_time.elapsed().as_secs_f32() >= update_interval {
                self.updates_per_second =
                    self.update_count as f32 / self.last_ups_time.elapsed().as_secs_f32();
                self.update_count = 0;
                self.last_ups_time = std::time::Instant::now();
                stats_changed = true;
            }
        }

        // Calculate how much simulated time we processed
        let simulated_time_processed = initial_delta - delta;

        // Only increment last_update_time by the actual (unscaled) time that corresponds
        // to the simulated time we processed
        if self.speed_multiplier > 0.0 {
            let actual_time_processed = simulated_time_processed.div_f32(self.speed_multiplier);
            self.last_update_time += actual_time_processed;
        }

        stats_changed
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

    fn calculate_center_of_mass(&self) -> Vector {
        if self.asteroids.is_empty() {
            return Vector { x: 0.0, y: 0.0 };
        }

        let mut total_mass = 0.0;
        let mut weighted_pos = Vector { x: 0.0, y: 0.0 };

        for asteroid in &self.asteroids {
            let mass = asteroid.size(); // size represents area/mass
            total_mass += mass;
            weighted_pos = weighted_pos + asteroid.pos() * mass;
        }

        weighted_pos / total_mass
    }

    fn calculate_mass_variance(&self, center: Vector) -> f32 {
        if self.asteroids.is_empty() {
            return 0.0;
        }

        let mut total_mass = 0.0;
        let mut weighted_variance = 0.0;

        for asteroid in &self.asteroids {
            let mass = asteroid.size();
            let distance = (asteroid.pos() - center).length();
            total_mass += mass;
            weighted_variance += mass * distance * distance;
        }

        if total_mass > 0.0 {
            (weighted_variance / total_mass).sqrt()
        } else {
            0.0
        }
    }

    fn cleanup_distant_asteroids(&mut self) {
        let center = self.calculate_center_of_mass();
        let std_dev = self.calculate_mass_variance(center);
        let threshold = std_dev * CLEANUP_THRESHOLD_MULTIPLIER;

        self.asteroids
            .retain(|asteroid| (asteroid.pos() - center).length() <= threshold);
    }

    fn increase_speed(&mut self) {
        self.speed_multiplier *= 1.5;
        // Cap at 10x
        if self.speed_multiplier > 10.0 {
            self.speed_multiplier = 10.0;
        }
    }

    fn decrease_speed(&mut self) {
        self.speed_multiplier /= 1.5;
        // Minimum 0.1x
        if self.speed_multiplier < 0.1 {
            self.speed_multiplier = 0.1;
        }
    }

    fn reset_speed(&mut self) {
        self.speed_multiplier = 1.0;
    }

    fn get_speed_multiplier(&self) -> f32 {
        self.speed_multiplier
    }
}

#[derive(Default)]
struct InputState {
    camera_pos: Vector,
    camera_vel: Vector,

    cursor_pos: Vector,

    // Asteroid creation state
    creating_asteroid: bool,
    asteroid_start_pos: Vector,
    asteroid_size: f32,
    asteroid_hold_time: f32,

    // Input state
    keys_pressed: HashSet<KeyCode>,
    mouse_buttons_pressed: HashSet<MouseButton>,

    // Camera tracking
    camera_tracking: bool,
}

impl InputState {
    fn new() -> Self {
        Self {
            asteroid_size: 1.0,
            ..Default::default()
        }
    }

    fn start_creating_asteroid(&mut self, screen_pos: Vector) {
        self.creating_asteroid = true;
        self.asteroid_start_pos = screen_pos;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;
    }

    fn update_asteroid_size(&mut self, dt: f32) {
        if self.creating_asteroid && self.mouse_buttons_pressed.contains(&MouseButton::Left) {
            self.asteroid_hold_time += dt;
            // Quadratic growth: size = 1 + (10*t)^2 for faster growth
            let scaled_time = self.asteroid_hold_time * 10.0;
            self.asteroid_size = 1.0 + scaled_time * scaled_time;
        }
    }

    fn finish_creating_asteroid(&mut self, screen_pos: Vector) -> (Vector, Vector, f32) {
        // Convert screen positions to world positions
        let world_start_pos = self.camera_pos + self.asteroid_start_pos;
        let world_end_pos = self.camera_pos + screen_pos;

        let pos = world_start_pos;
        // Add camera velocity to asteroid velocity
        let vel = (world_end_pos - world_start_pos) + self.camera_vel;
        let size = self.asteroid_size;

        self.creating_asteroid = false;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;

        (pos, vel, size)
    }
}

impl InputState {
    fn update_camera(&mut self, dt: f32) {
        // Set velocity first (in pixels per second)
        self.camera_vel = Vector { x: 0.0, y: 0.0 };

        if self.keys_pressed.contains(&KeyCode::KeyW) {
            self.camera_vel.y -= CAMERA_SPEED;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            self.camera_vel.y += CAMERA_SPEED;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            self.camera_vel.x -= CAMERA_SPEED;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            self.camera_vel.x += CAMERA_SPEED;
        }

        // Update position based on velocity and time
        self.camera_pos = self.camera_pos + self.camera_vel * dt;
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

    // Frame timing
    last_frame_time: std::time::Instant,
    frame_count: u32,
    last_fps_time: std::time::Instant,
    frames_per_second: f32,
    stats_changed: bool,

    // Random spawning
    random_spawn_timer: f32,
    random_spawn_hold_time: f32,

    // Window state
    window_visible: bool,
}

impl RunningState {
    fn new(window: Window) -> Self {
        // Pin the window on the heap => stable address.
        let window = Box::pin(window);

        // Safe: getting a shared reference to a pinned value is fine.
        let w_ref: &Window = window.as_ref().get_ref();

        let size = w_ref.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, w_ref);

        // Use PixelsBuilder to set present mode to Immediate (no vsync) for maximum FPS
        let pixels = pixels::PixelsBuilder::new(size.width, size.height, surface)
            .present_mode(pixels::wgpu::PresentMode::Mailbox)
            .build()
            .unwrap();

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

        let framebuffer = FrameBuffer::new(pixels, size.width, size.height);

        let now = std::time::Instant::now();
        Self {
            framebuffer,
            window,
            world: WorldState::new(),
            input: InputState::new(),
            last_frame_time: now,
            frame_count: 0,
            last_fps_time: now,
            frames_per_second: 0.0,
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
        if self.input.creating_asteroid {
            // Convert screen position to world position for drawing
            let world_pos = self.input.camera_pos + self.input.asteroid_start_pos;
            let preview = Asteroid::new(
                world_pos,
                Vector { x: 0.0, y: 0.0 },
                self.input.asteroid_size,
            );
            preview.draw(&mut self.framebuffer, Color::WHITE);
        }

        // Draw stats text in screen space (top-left corner)
        let stats_text = format!(
            "FPS: {} | UPS: {} | Asteroids: {}",
            self.frames_per_second as u32,
            self.world.updates_per_second as u32,
            self.world.asteroids.len()
        );
        let text_pos = Vector { x: 10.0, y: 10.0 };
        self.framebuffer
            .draw_text(&stats_text, text_pos, 16.0, Color::WHITE);

        // Draw speed multiplier in top-right corner
        let speed_text = format!("Speed: {:.1}x", self.world.get_speed_multiplier());
        let window_size = self.window().inner_size();
        // Approximate text width (assuming ~10 pixels per character for 16pt font)
        let text_width = speed_text.len() as f32 * 10.0;
        let speed_pos = Vector {
            x: window_size.width as f32 - text_width - 10.0,
            y: 10.0,
        };
        self.framebuffer
            .draw_text(&speed_text, speed_pos, 16.0, Color::WHITE);

        self.framebuffer.render().unwrap();

        // Track frames per second
        self.frame_count += 1;
        let update_interval = 1.0 / STATS_UPDATE_RATE;
        if self.last_fps_time.elapsed().as_secs_f32() >= update_interval {
            self.frames_per_second =
                self.frame_count as f32 / self.last_fps_time.elapsed().as_secs_f32();
            self.frame_count = 0;
            self.last_fps_time = std::time::Instant::now();
            self.stats_changed = true;
        }
    }

    fn on_press(&mut self) {
        if !self.input.creating_asteroid {
            // First click - start creating asteroid at this screen position
            let screen_pos = self.input.cursor_pos;
            self.input.start_creating_asteroid(screen_pos);
        } else if !self
            .input
            .mouse_buttons_pressed
            .contains(&MouseButton::Left)
        {
            // Second click (button not held) - finish creating asteroid with velocity
            let screen_pos = self.input.cursor_pos;
            let (pos, vel, size) = self.input.finish_creating_asteroid(screen_pos);
            self.world.spawn_asteroid(pos, vel, size);
        }
    }

    fn spawn_random_asteroid(&mut self) {
        // Get actual window size
        let window_size = self.window().inner_size();
        let width = window_size.width as f32;
        let height = window_size.height as f32;

        // Random position within visible screen area
        let x = self.input.camera_pos.x + fastrand::f32() * width;
        let y = self.input.camera_pos.y + fastrand::f32() * height;
        let pos = Vector { x, y };

        // Random velocity: random direction and magnitude (10-50 pixels/sec)
        let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;
        let speed = 10.0 + fastrand::f32() * 40.0;
        let random_vel = Vector {
            x: angle.cos() * speed,
            y: angle.sin() * speed,
        };

        // Add camera velocity to asteroid velocity
        let vel = random_vel + self.input.camera_vel;

        // Random size (5-30)
        let size = 5.0 + fastrand::f32() * 25.0;

        self.world.spawn_asteroid(pos, vel, size);
    }

    fn update(&mut self) {
        // Calculate actual frame time
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Update camera position every frame (if not tracking)
        if !self.input.camera_tracking {
            self.input.update_camera(dt);
        } else {
            // Reset camera velocity when not manually moving
            self.input.camera_vel = Vector { x: 0.0, y: 0.0 };
        }

        // Update world physics
        if self.world.update() {
            self.stats_changed = true;
        }

        // Update camera to center of mass if tracking is enabled
        if self.input.camera_tracking {
            let center = self.world.calculate_center_of_mass();
            let window_size = self.window().inner_size();
            let half_width = window_size.width as f32 / 2.0;
            let half_height = window_size.height as f32 / 2.0;
            let new_camera_pos = center
                - Vector {
                    x: half_width,
                    y: half_height,
                };

            // Calculate camera velocity from tracking movement
            self.input.camera_vel = (new_camera_pos - self.input.camera_pos) / dt;
            self.input.camera_pos = new_camera_pos;
        }

        // Update asteroid size while button held (quadratic growth)
        self.input.update_asteroid_size(dt);

        // Random spawning when R key is held
        if self.input.keys_pressed.contains(&KeyCode::KeyR) {
            self.random_spawn_timer += dt;
            self.random_spawn_hold_time += dt;

            // Calculate current spawn rate based on hold time
            // Starts at 10/sec, increases by 2/sec for each second held
            let current_spawn_rate = RANDOM_SPAWN_RATE_INITIAL
                + (self.random_spawn_hold_time * RANDOM_SPAWN_RATE_INCREASE);

            let spawn_interval = 1.0 / current_spawn_rate;
            while self.random_spawn_timer >= spawn_interval {
                self.spawn_random_asteroid();
                self.random_spawn_timer -= spawn_interval;
            }
        } else {
            // Reset timers when key is released
            self.random_spawn_timer = 0.0;
            self.random_spawn_hold_time = 0.0;
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        // Update physics continuously - called at start of every event loop iteration
        // This runs even when window is hidden/minimized (unlike about_to_wait which can be blocked by present)
        let AppState::Running(running) = &mut self.state else {
            return;
        };

        running.update();

        // Request redraw if window is visible
        if running.window_visible {
            running.window().request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, AppState::Running(_)) {
            return;
        }

        // Get primary monitor to calculate half screen size
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
                running.input.cursor_pos = Vector {
                    x: position.x as f32,
                    y: position.y as f32,
                };
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

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            running.input.keys_pressed.insert(keycode);

                            // Handle T key to start tracking
                            if keycode == KeyCode::KeyT {
                                running.input.camera_tracking = true;
                            }

                            // Handle +/- keys to adjust simulation speed
                            // + key (Shift + =) increases speed
                            let shift_pressed =
                                running.input.keys_pressed.contains(&KeyCode::ShiftLeft)
                                    || running.input.keys_pressed.contains(&KeyCode::ShiftRight);

                            if (keycode == KeyCode::Equal && shift_pressed)
                                || keycode == KeyCode::NumpadAdd
                            {
                                running.world.increase_speed();
                                running.stats_changed = true;
                            }
                            // = key (without shift) resets speed to 1.0x
                            else if keycode == KeyCode::Equal {
                                running.world.reset_speed();
                                running.stats_changed = true;
                            }
                            // - key decreases speed
                            if keycode == KeyCode::Minus || keycode == KeyCode::NumpadSubtract {
                                running.world.decrease_speed();
                                running.stats_changed = true;
                            }

                            // Stop tracking when WASD is pressed
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
