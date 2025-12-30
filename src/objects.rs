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
        fb.draw_circle(self.pos, self.radius(), color);
    }

    pub fn update(&mut self, others: &Vec<Asteroid>, step: f32) {
        let mut acc = Vector { x: 0.0, y: 0.0 };
        for asteroid in others {
            let direction = asteroid.pos - self.pos;
            let distance = direction.length();
            if distance < (self.radius() + asteroid.radius()) {
                continue;
            }
            let force_magnitude = asteroid.size / distance;
            let force = direction.norm() * force_magnitude;
            acc += force;
        }
        self.vel = acc * step + self.vel;
        self.pos += self.vel * step;
    }

    pub fn collides_with(&self, other: &Asteroid) -> bool {
        (self.pos - other.pos).length() <= (self.radius() + other.radius())
    }

    pub fn merge_with(&self, other: &Asteroid) -> Asteroid {
        let mass1 = self.size;
        let mass2 = other.size;
        let total_mass = mass1 + mass2;

        let momentum_x = mass1 * self.vel.x + mass2 * other.vel.x;
        let momentum_y = mass1 * self.vel.y + mass2 * other.vel.y;

        let new_vel = Vector {
            x: momentum_x / total_mass,
            y: momentum_y / total_mass,
        };

        let new_size = self.size + other.size;

        let new_pos = Vector {
            x: (mass1 * self.pos.x + mass2 * other.pos.x) / total_mass,
            y: (mass1 * self.pos.y + mass2 * other.pos.y) / total_mass,
        };

        Asteroid::new(new_pos, new_vel, new_size)
    }
}
