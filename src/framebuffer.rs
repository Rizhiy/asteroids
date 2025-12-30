use crate::color::Color;
use crate::vector::Vector;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use pixels::Pixels;

pub struct FrameBuffer {
    pixels: Pixels<'static>,
    width: u32,
    height: u32,
    camera_pos: Vector,
    zoom: f32,
    font: Font,
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
            camera_pos: Vector { x: 0.0, y: 0.0 },
            zoom: 1.0,
            font,
        }
    }

    pub fn set_camera_pos(&mut self, camera_pos: Vector) {
        self.camera_pos = camera_pos;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
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

    pub fn draw_circle(&mut self, world_pos: Vector, world_radius: f32, color: Color) {
        let screen_center = Vector {
            x: self.width as f32 / 2.0,
            y: self.height as f32 / 2.0,
        };
        let screen_pos = (world_pos - self.camera_pos) * self.zoom + screen_center;
        let screen_radius = world_radius * self.zoom;

        let ceil_radius = screen_radius.ceil() as i32;
        let center_x = screen_pos.x.round() as i32;
        let center_y = screen_pos.y.round() as i32;

        for y_offset in -ceil_radius..=ceil_radius {
            for x_offset in -ceil_radius..=ceil_radius {
                let dx = x_offset as f32;
                let dy = y_offset as f32;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance <= screen_radius {
                    self.set_screen_pixel(center_x + x_offset, center_y + y_offset, color);
                }
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

    pub fn draw_text(&mut self, text: &str, screen_pos: Vector, font_size: f32, color: Color) {
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
}
