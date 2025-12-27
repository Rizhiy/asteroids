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

    pub fn update(&mut self, others: &Vec<Asteroid>, step: f32) {
        let mut acc = Vector { x: 0.0, y: 0.0 };
        for asteroid in others {
            let direction = asteroid.pos - self.pos;
            let distance = direction.length();
            if distance < (self.radius() + asteroid.radius()) {
                continue;
            }
            // Not multiplying by self mass, since we would need to divide by it later
            let force_magnitude = asteroid.size / (distance * distance);
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

#[derive(Default, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
}
