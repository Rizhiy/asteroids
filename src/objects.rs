use crate::color::Color;
use glam::{Vec2, vec2};
use std::f32::consts::PI;

#[derive(Default, Clone, Copy, Debug)]
pub struct Asteroid {
    pos: Vec2,
    vel: Vec2,
    size: f32,
}

impl Asteroid {
    pub fn new(pos: Vec2, vel: Vec2, size: f32) -> Self {
        Self { pos, vel, size }
    }
    pub fn pos(&self) -> Vec2 {
        self.pos
    }

    pub fn vel(&self) -> Vec2 {
        self.vel
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
        let mut acc = vec2(0.0, 0.0);
        for asteroid in others {
            let direction = asteroid.pos - self.pos;
            let distance = direction.length();
            if distance < (self.radius() + asteroid.radius()) {
                continue;
            }
            let force_magnitude = asteroid.size / distance;
            let force = direction.normalize() * force_magnitude;
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

        let new_vel = vec2(momentum_x / total_mass, momentum_y / total_mass);

        let new_size = self.size + other.size;

        let new_pos = vec2(
            (mass1 * self.pos.x + mass2 * other.pos.x) / total_mass,
            (mass1 * self.pos.y + mass2 * other.pos.y) / total_mass,
        );

        Asteroid::new(new_pos, new_vel, new_size)
    }
}
