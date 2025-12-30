use crate::color::Color;
use crate::vector::Vector;
use std::f32::consts::PI;

#[derive(Default, Clone, Copy, Debug)]
pub struct Asteroid {
    pos: Vector,
    vel: Vector,
    size: f32,
}

impl Asteroid {
    pub fn new(pos: Vector, vel: Vector, size: f32) -> Self {
        Self { pos, vel, size }
    }
    pub fn pos(&self) -> Vector {
        self.pos
    }
    pub fn radius(self) -> f32 {
        self.size.sqrt() / PI
    }

    pub fn size(self) -> f32 {
        self.size
    }

    pub fn draw(&self, fb: &mut crate::framebuffer::FrameBuffer, color: Color) {
        let radius = self.radius();
        let ceil_radius = radius.ceil() as i32;

        for x_offset in -ceil_radius..ceil_radius {
            for y_offset in -ceil_radius..ceil_radius {
                let pixel_pos = self.pos
                    + Vector {
                        x: x_offset as f32,
                        y: y_offset as f32,
                    };
                if (pixel_pos - self.pos).length() <= radius {
                    fb.set_pixel(pixel_pos, color);
                }
            }
        }
    }

    pub fn update(&mut self, others: &Vec<Asteroid>, step: f32) {
        let mut acc = Vector { x: 0.0, y: 0.0 };
        for asteroid in others {
            let direction = asteroid.pos - self.pos;
            let distance = direction.length();
            if distance < (self.radius() + asteroid.radius()) {
                continue;
            }
            // Not multiplying by self mass, since we would need to divide by it later
            let force_magnitude = asteroid.size / distance;
            let force = direction.norm() * force_magnitude;
            acc += force;
        }
        self.vel = acc * step + self.vel;
        self.pos += self.vel * step;
    }

    /// Check if this asteroid collides with another
    pub fn collides_with(&self, other: &Asteroid) -> bool {
        (self.pos - other.pos).length() <= (self.radius() + other.radius())
    }

    /// Merge this asteroid with another, preserving momentum and size
    pub fn merge_with(&self, other: &Asteroid) -> Asteroid {
        // Size represents area (mass is proportional to area in 2D)
        let mass1 = self.size;
        let mass2 = other.size;
        let total_mass = mass1 + mass2;

        // Preserve momentum: p = m * v
        let momentum_x = mass1 * self.vel.x + mass2 * other.vel.x;
        let momentum_y = mass1 * self.vel.y + mass2 * other.vel.y;

        let new_vel = Vector {
            x: momentum_x / total_mass,
            y: momentum_y / total_mass,
        };

        // Combine sizes (areas)
        let new_size = self.size + other.size;

        // Position at center of mass
        let new_pos = Vector {
            x: (mass1 * self.pos.x + mass2 * other.pos.x) / total_mass,
            y: (mass1 * self.pos.y + mass2 * other.pos.y) / total_mass,
        };

        Asteroid::new(new_pos, new_vel, new_size)
    }
}
