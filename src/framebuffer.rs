use crate::color::Color;
use crate::vector::Vector;
use pixels::Pixels;

/// Wrapper around the pixel frame buffer with helper methods
pub struct FrameBuffer {
    pixels: Pixels<'static>,
    width: u32,
    height: u32,
    camera_pos: Vector,
}

impl FrameBuffer {
    pub fn new(pixels: Pixels<'static>, width: u32, height: u32) -> Self {
        Self {
            pixels,
            width,
            height,
            camera_pos: Vector { x: 0.0, y: 0.0 },
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
        self.pixels.resize_surface(width, height).map_err(|e| format!("{:?}", e))?;
        self.pixels.resize_buffer(width, height).map_err(|e| format!("{:?}", e))
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
}
