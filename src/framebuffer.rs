use crate::color::Color;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use glam::{Vec2, vec2};
use pixels::Pixels;
use std::collections::HashSet;
use winit::{event::MouseButton, keyboard::KeyCode};

const CAMERA_SPEED: f32 = 300.0;

pub struct FrameBuffer {
    pixels: Pixels<'static>,
    width: u32,
    height: u32,
    font: Font,
    // Input state fields
    pub camera_pos: Vec2,
    pub camera_vel: Vec2,
    pub cursor_pos: Vec2,
    pub creating_asteroid: bool,
    pub asteroid_start_pos: Vec2,
    pub asteroid_size: f32,
    pub asteroid_hold_time: f32,
    pub keys_pressed: HashSet<KeyCode>,
    pub mouse_buttons_pressed: HashSet<MouseButton>,
    pub speed_multiplier: f32,
    pub previous_speed: f32,
    pub camera_tracking: bool,
    pub zoom: f32,
}

impl FrameBuffer {
    pub fn new(pixels: Pixels<'static>, width: u32, height: u32) -> Self {
        const FONT_DATA: &[u8] = include_bytes!("../static/fonts/RobotoMono-Regular.ttf");
        let font = Font::from_bytes(FONT_DATA, FontSettings::default())
            .expect("Failed to load embedded font");

        Self {
            pixels,
            width,
            height,
            font,
            camera_pos: vec2(0.0, 0.0),
            camera_vel: vec2(0.0, 0.0),
            cursor_pos: vec2(0.0, 0.0),
            creating_asteroid: false,
            asteroid_start_pos: vec2(0.0, 0.0),
            asteroid_size: 1.0,
            asteroid_hold_time: 0.0,
            keys_pressed: HashSet::new(),
            mouse_buttons_pressed: HashSet::new(),
            speed_multiplier: 1.0,
            previous_speed: 1.0,
            camera_tracking: false,
            zoom: 1.0,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.width = width;
        self.height = height;
        self.pixels
            .resize_surface(width, height)
            .map_err(|e| format!("{:?}", e))?;
        self.pixels
            .resize_buffer(width, height)
            .map_err(|e| format!("{:?}", e))
    }

    fn set_screen_pixel(&mut self, screen_x: i32, screen_y: i32, color: Color) {
        if screen_x < 0
            || screen_x >= self.width as i32
            || screen_y < 0
            || screen_y >= self.height as i32
        {
            return;
        }

        let frame = self.pixels.frame_mut();
        let index = ((screen_y as u32 * self.width + screen_x as u32) * 4) as usize;
        frame[index] = color.r;
        frame[index + 1] = color.g;
        frame[index + 2] = color.b;
        frame[index + 3] = color.a;
    }

    fn add_screen_pixel(&mut self, screen_x: i32, screen_y: i32, color: Color) {
        if screen_x < 0
            || screen_x >= self.width as i32
            || screen_y < 0
            || screen_y >= self.height as i32
        {
            return;
        }

        let frame = self.pixels.frame_mut();
        let index = ((screen_y as u32 * self.width + screen_x as u32) * 4) as usize;
        frame[index] = color.r;
        frame[index + 1] = color.g;
        frame[index + 2] = color.b;
        frame[index + 3] = (frame[index + 3] as u16 + color.a as u16).min(255) as u8;
    }

    pub fn draw_circle(&mut self, world_pos: Vec2, world_radius: f32, color: Color) {
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        let screen_pos = (world_pos - self.camera_pos) * self.zoom + screen_center;
        let screen_radius = world_radius * self.zoom;

        // For very small asteroids, just draw a single dimmed pixel
        if screen_radius < 0.5 {
            let center_x = screen_pos.x.round() as i32;
            let center_y = screen_pos.y.round() as i32;

            // Coverage based on area: circle area / pixel area = pi * r^2
            let coverage = (std::f32::consts::PI * screen_radius * screen_radius).min(1.0);

            let aa_color = Color {
                r: color.r,
                g: color.g,
                b: color.b,
                a: (color.a as f32 * coverage) as u8,
            };
            self.add_screen_pixel(center_x, center_y, aa_color);
            return;
        }

        let ceil_radius = screen_radius.ceil() as i32;
        let center_x = screen_pos.x.round() as i32;
        let center_y = screen_pos.y.round() as i32;

        for y_offset in -ceil_radius..=ceil_radius {
            for x_offset in -ceil_radius..=ceil_radius {
                let dx = x_offset as f32;
                let dy = y_offset as f32;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance > screen_radius + 1.0 {
                    continue;
                }

                // Calculate coverage for anti-aliasing
                let coverage = if distance < screen_radius {
                    1.0
                } else {
                    (screen_radius + 1.0 - distance).max(0.0)
                };

                if coverage == 0.0 {
                    continue;
                }

                let aa_color = Color {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: (color.a as f32 * coverage) as u8,
                };
                self.set_screen_pixel(center_x + x_offset, center_y + y_offset, aa_color);
            }
        }
    }

    pub fn clear(&mut self, color: Color) {
        let frame = self.pixels.frame_mut();
        for px in frame.chunks_exact_mut(4) {
            px[0] = color.r;
            px[1] = color.g;
            px[2] = color.b;
            px[3] = color.a;
        }
    }

    pub fn render(&mut self) -> Result<(), pixels::Error> {
        self.pixels.render()
    }

    pub fn draw_text(&mut self, text: &str, screen_pos: Vec2, font_size: f32, color: Color) {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: screen_pos.x,
            y: screen_pos.y,
            ..Default::default()
        });
        layout.append(&[&self.font], &TextStyle::new(text, font_size, 0));

        for glyph in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_config(glyph.key);

            let glyph_x = glyph.x as i32;
            let glyph_y = glyph.y as i32;

            for (i, &coverage) in bitmap.iter().enumerate() {
                if coverage == 0 {
                    continue;
                }

                let pixel_x = glyph_x + (i % metrics.width) as i32;
                let pixel_y = glyph_y + (i / metrics.width) as i32;

                if pixel_x < 0
                    || pixel_x >= self.width as i32
                    || pixel_y < 0
                    || pixel_y >= self.height as i32
                {
                    continue;
                }

                let frame = self.pixels.frame_mut();
                let index = ((pixel_y as u32 * self.width + pixel_x as u32) * 4) as usize;
                frame[index] = color.r;
                frame[index + 1] = color.g;
                frame[index + 2] = color.b;
                frame[index + 3] = coverage;
            }
        }
    }

    // Input methods
    pub fn adjust_speed(&mut self, factor: f32) {
        self.speed_multiplier *= factor;
        self.speed_multiplier = self.speed_multiplier.clamp(0.1, 100.0);
    }

    pub fn reset_speed(&mut self) {
        self.speed_multiplier = 1.0;
    }

    pub fn toggle_pause(&mut self) {
        if self.speed_multiplier == 0.0 {
            self.speed_multiplier = self.previous_speed;
        } else {
            self.previous_speed = self.speed_multiplier;
            self.speed_multiplier = 0.0;
        }
    }

    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        (screen_pos - screen_center) / self.zoom + self.camera_pos
    }

    pub fn apply_zoom(&mut self, cursor_pos: Vec2, zoom_factor: f32) {
        let world_pos_before = self.screen_to_world(cursor_pos);
        self.zoom *= zoom_factor;
        self.zoom = self.zoom.clamp(0.01, 10.0);
        let world_pos_after = self.screen_to_world(cursor_pos);
        self.camera_pos += world_pos_before - world_pos_after;
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
    }

    pub fn start_creating_asteroid(&mut self, screen_pos: Vec2) {
        self.creating_asteroid = true;
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        self.asteroid_start_pos = (screen_pos - screen_center) / self.zoom + self.camera_pos;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;
    }

    pub fn update_asteroid_size(&mut self, dt: f32) {
        if self.creating_asteroid && self.mouse_buttons_pressed.contains(&MouseButton::Left) {
            self.asteroid_hold_time += dt;
            let scaled_time = self.asteroid_hold_time * 10.0;
            self.asteroid_size = 1.0 + scaled_time * scaled_time;
        }
    }

    pub fn finish_creating_asteroid(
        &mut self,
        screen_pos: Vec2,
        actual_speed: f32,
    ) -> (Vec2, Vec2, f32) {
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        let world_end_pos = (screen_pos - screen_center) / self.zoom + self.camera_pos;

        let pos = self.asteroid_start_pos;
        let base_vel = (world_end_pos - self.asteroid_start_pos) + self.camera_vel;
        let vel = base_vel * actual_speed;
        let size = self.asteroid_size;

        self.creating_asteroid = false;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;

        (pos, vel, size)
    }

    pub fn update_camera(&mut self, dt: f32) {
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
