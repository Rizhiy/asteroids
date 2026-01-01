use crate::color::Color;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use glam::{Vec2, vec2};
use image::RgbaImage;
use pixels::Pixels;
use std::collections::HashSet;
use winit::{event::MouseButton, keyboard::KeyCode};

const CAMERA_SPEED: f32 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Manual,
    TrackingCenterOfMass,
    ShipControl,
}

impl CameraMode {
    pub fn name(&self) -> &str {
        match self {
            CameraMode::Manual => "Manual",
            CameraMode::TrackingCenterOfMass => "Tracking",
            CameraMode::ShipControl => "Ship",
        }
    }
}

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
    pub asteroid_growing: bool,
    pub asteroid_screen_pos: Vec2,
    pub asteroid_screen_vel: Vec2,
    pub asteroid_size: f32,
    pub asteroid_hold_time: f32,
    pub keys_pressed: HashSet<KeyCode>,
    pub mouse_buttons_pressed: HashSet<MouseButton>,
    pub speed_multiplier: f32,
    pub previous_speed: f32,
    pub camera_mode: CameraMode,
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
            asteroid_growing: false,
            asteroid_screen_pos: vec2(0.0, 0.0),
            asteroid_screen_vel: vec2(0.0, 0.0),
            asteroid_size: 1.0,
            asteroid_hold_time: 0.0,
            keys_pressed: HashSet::new(),
            mouse_buttons_pressed: HashSet::new(),
            speed_multiplier: 1.0,
            previous_speed: 1.0,
            camera_mode: CameraMode::Manual,
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

    pub fn set_pixel(&mut self, screen_x: i32, screen_y: i32, color: Color) {
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
                self.set_pixel(center_x + x_offset, center_y + y_offset, aa_color);
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
        self.zoom = self.zoom.clamp(0.001, 10.0);
        let world_pos_after = self.screen_to_world(cursor_pos);
        if self.camera_mode == CameraMode::Manual {
            self.camera_pos += world_pos_before - world_pos_after;
        }
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
    }

    pub fn start_creating_asteroid(&mut self, screen_pos: Vec2) {
        self.creating_asteroid = true;
        self.asteroid_growing = true;
        self.asteroid_screen_pos = screen_pos;
        self.asteroid_screen_vel = vec2(0.0, 0.0);
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;
    }

    pub fn update_asteroid_size(&mut self, dt: f32) {
        if self.creating_asteroid && self.asteroid_growing {
            if self.mouse_buttons_pressed.contains(&MouseButton::Left) {
                self.asteroid_hold_time += dt;
                let scaled_time = self.asteroid_hold_time * 10.0;
                self.asteroid_size = 1.0 + scaled_time * scaled_time;
                self.asteroid_screen_pos = self.cursor_pos;
            } else {
                // Button released, lock size and position
                self.asteroid_growing = false;
            }
        }
    }

    pub fn finish_creating_asteroid(
        &mut self,
        screen_pos: Vec2,
        actual_speed: f32,
    ) -> (Vec2, Vec2, f32) {
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);

        // Calculate velocity in screen space
        self.asteroid_screen_vel = screen_pos - self.asteroid_screen_pos;

        // Convert position from screen to world
        let world_pos = (self.asteroid_screen_pos - screen_center) / self.zoom + self.camera_pos;

        // Convert velocity from screen to world
        let mut world_vel = self.asteroid_screen_vel / self.zoom + self.camera_vel;

        // Divide by actual_speed so faster simulation = smaller velocity in world units
        if actual_speed > 0.0 {
            world_vel /= actual_speed;
        }

        let size = self.asteroid_size;

        self.creating_asteroid = false;
        self.asteroid_growing = false;
        self.asteroid_size = 1.0;
        self.asteroid_hold_time = 0.0;

        (world_pos, world_vel, size)
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

    pub fn draw_screen_line(&mut self, p0: Vec2, p1: Vec2, color: Color) {
        let delta = p1 - p0;
        let length = delta.length();

        if length < 1.0 {
            self.set_pixel(p0.x as i32, p0.y as i32, color);
            return;
        }

        let dir = delta / length;
        let num_steps = length.ceil() as i32;

        for i in 0..=num_steps {
            let t = i as f32;
            let pos = p0 + dir * t;
            self.set_pixel(pos.x as i32, pos.y as i32, color);
        }
    }

    pub fn draw_screen_triangle(&mut self, p0: Vec2, p1: Vec2, p2: Vec2, color: Color) {
        // Find bounding box
        let min_x = p0.x.min(p1.x).min(p2.x).floor() as i32;
        let max_x = p0.x.max(p1.x).max(p2.x).ceil() as i32;
        let min_y = p0.y.min(p1.y).min(p2.y).floor() as i32;
        let max_y = p0.y.max(p1.y).max(p2.y).ceil() as i32;

        // For each pixel in bounding box, check if it's inside the triangle
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = vec2(x as f32 + 0.5, y as f32 + 0.5);

                // Barycentric coordinates
                let v0 = p2 - p0;
                let v1 = p1 - p0;
                let v2 = p - p0;

                let dot00 = v0.dot(v0);
                let dot01 = v0.dot(v1);
                let dot02 = v0.dot(v2);
                let dot11 = v1.dot(v1);
                let dot12 = v1.dot(v2);

                let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
                let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
                let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

                // Check if point is inside triangle
                if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    pub fn draw_screen_rectangle(&mut self, top_left: Vec2, width: f32, height: f32, color: Color) {
        let top_right = vec2(top_left.x + width, top_left.y);
        let bottom_left = vec2(top_left.x, top_left.y + height);
        let bottom_right = vec2(top_left.x + width, top_left.y + height);

        // Draw two triangles to form a rectangle
        self.draw_screen_triangle(top_left, top_right, bottom_left, color);
        self.draw_screen_triangle(top_right, bottom_right, bottom_left, color);
    }

    pub fn draw_sprite(
        &mut self,
        sprite: &RgbaImage,
        world_pos: Vec2,
        scale: f32,
        orientation: f32,
    ) {
        let screen_center = vec2(self.width as f32 / 2.0, self.height as f32 / 2.0);
        let screen_pos = (world_pos - self.camera_pos) * self.zoom + screen_center;

        let (sprite_width, sprite_height) = sprite.dimensions();
        let scaled_size = scale * self.zoom;

        // Calculate screen space bounding box
        let half_width = sprite_width as f32 / 2.0 * scaled_size;
        let half_height = sprite_height as f32 / 2.0 * scaled_size;
        let max_radius = (half_width * half_width + half_height * half_height).sqrt();

        let min_x = (screen_pos.x - max_radius).floor() as i32;
        let max_x = (screen_pos.x + max_radius).ceil() as i32;
        let min_y = (screen_pos.y - max_radius).floor() as i32;
        let max_y = (screen_pos.y + max_radius).ceil() as i32;

        let cos_angle = orientation.cos();
        let sin_angle = orientation.sin();

        // Iterate over all screen pixels in the bounding box
        for screen_y in min_y..=max_y {
            for screen_x in min_x..=max_x {
                // Screen pixel offset from sprite center
                let dx = screen_x as f32 - screen_pos.x;
                let dy = screen_y as f32 - screen_pos.y;

                // Rotate back to sprite space
                let sprite_x =
                    (dx * cos_angle + dy * sin_angle) / scaled_size + sprite_width as f32 / 2.0;
                let sprite_y =
                    (-dx * sin_angle + dy * cos_angle) / scaled_size + sprite_height as f32 / 2.0;

                // Check if within sprite bounds
                if sprite_x < 0.0
                    || sprite_x >= sprite_width as f32
                    || sprite_y < 0.0
                    || sprite_y >= sprite_height as f32
                {
                    continue;
                }

                // Bilinear interpolation
                let x0 = sprite_x.floor() as u32;
                let y0 = sprite_y.floor() as u32;
                let x1 = (x0 + 1).min(sprite_width - 1);
                let y1 = (y0 + 1).min(sprite_height - 1);

                let fx = sprite_x - x0 as f32;
                let fy = sprite_y - y0 as f32;

                let p00 = sprite.get_pixel(x0, y0);
                let p10 = sprite.get_pixel(x1, y0);
                let p01 = sprite.get_pixel(x0, y1);
                let p11 = sprite.get_pixel(x1, y1);

                // Interpolate each channel
                let r = ((p00[0] as f32 * (1.0 - fx) + p10[0] as f32 * fx) * (1.0 - fy)
                    + (p01[0] as f32 * (1.0 - fx) + p11[0] as f32 * fx) * fy)
                    as u8;
                let g = ((p00[1] as f32 * (1.0 - fx) + p10[1] as f32 * fx) * (1.0 - fy)
                    + (p01[1] as f32 * (1.0 - fx) + p11[1] as f32 * fx) * fy)
                    as u8;
                let b = ((p00[2] as f32 * (1.0 - fx) + p10[2] as f32 * fx) * (1.0 - fy)
                    + (p01[2] as f32 * (1.0 - fx) + p11[2] as f32 * fx) * fy)
                    as u8;
                let a = ((p00[3] as f32 * (1.0 - fx) + p10[3] as f32 * fx) * (1.0 - fy)
                    + (p01[3] as f32 * (1.0 - fx) + p11[3] as f32 * fx) * fy)
                    as u8;

                if a == 0 {
                    continue;
                }

                let color = Color { r, g, b, a };
                self.set_pixel(screen_x, screen_y, color);
            }
        }
    }
}
