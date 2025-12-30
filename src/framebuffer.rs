use crate::color::Color;
use crate::vector::Vector;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use pixels::Pixels;

/// Wrapper around the pixel frame buffer with helper methods
pub struct FrameBuffer {
    pixels: Pixels<'static>,
    width: u32,
    height: u32,
    camera_pos: Vector,
    font: Font,
}

impl FrameBuffer {
    pub fn new(pixels: Pixels<'static>, width: u32, height: u32) -> Self {
        // Load an embedded basic font for text rendering
        // Using a simple sans-serif font data embedded in the binary
        const FONT_DATA: &[u8] = include_bytes!("../static/fonts/RobotoMono-Regular.ttf");
        let font = Font::from_bytes(FONT_DATA, FontSettings::default())
            .expect("Failed to load embedded font");

        Self {
            pixels,
            width,
            height,
            camera_pos: Vector { x: 0.0, y: 0.0 },
            font,
        }
    }

    /// Set the camera position
    pub fn set_camera_pos(&mut self, camera_pos: Vector) {
        self.camera_pos = camera_pos;
    }

    /// Get the camera position
    pub fn camera_pos(&self) -> Vector {
        self.camera_pos
    }

    /// Resize both the surface and the buffer
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

    /// Set a pixel at world coordinates
    pub fn set_pixel(&mut self, world_pos: Vector, color: Color) {
        // Convert world coordinates to screen coordinates
        let screen_pos = world_pos - self.camera_pos;

        if screen_pos.x < 0.0
            || screen_pos.x >= self.width as f32
            || screen_pos.y < 0.0
            || screen_pos.y >= self.height as f32
        {
            return;
        }

        let frame = self.pixels.frame_mut();
        let index = ((screen_pos.y as u32 * self.width + screen_pos.x as u32) * 4) as usize;
        frame[index] = color.r;
        frame[index + 1] = color.g;
        frame[index + 2] = color.b;
        frame[index + 3] = color.a;
    }

    /// Clear the entire frame with a color
    pub fn clear(&mut self, color: Color) {
        let frame = self.pixels.frame_mut();
        for px in frame.chunks_exact_mut(4) {
            px[0] = color.r;
            px[1] = color.g;
            px[2] = color.b;
            px[3] = color.a;
        }
    }

    /// Render the frame to the screen
    pub fn render(&mut self) -> Result<(), pixels::Error> {
        self.pixels.render()
    }

    /// Draw text at the specified screen position
    ///
    /// # Arguments
    /// * `text` - The text string to render
    /// * `screen_pos` - The screen position (top-left corner of the text)
    /// * `font_size` - The font size in pixels
    /// * `color` - The color of the text
    pub fn draw_text(&mut self, text: &str, screen_pos: Vector, font_size: f32, color: Color) {
        // Set up layout for text rendering
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: screen_pos.x,
            y: screen_pos.y,
            ..Default::default()
        });
        layout.append(&[&self.font], &TextStyle::new(text, font_size, 0));

        // Render each glyph
        for glyph in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_config(glyph.key);

            // Position where this glyph should be drawn (screen space)
            let glyph_x = glyph.x as i32;
            let glyph_y = glyph.y as i32;

            // Draw each pixel of the glyph bitmap
            for (i, &coverage) in bitmap.iter().enumerate() {
                if coverage == 0 {
                    continue; // Skip transparent pixels
                }

                let pixel_x = glyph_x + (i % metrics.width) as i32;
                let pixel_y = glyph_y + (i / metrics.width) as i32;

                // Check bounds
                if pixel_x < 0
                    || pixel_x >= self.width as i32
                    || pixel_y < 0
                    || pixel_y >= self.height as i32
                {
                    continue;
                }

                // Set the pixel with coverage as alpha
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
