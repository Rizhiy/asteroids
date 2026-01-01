use crate::objects::Asteroid;
use glam::{Vec2, vec2};

const SHIP_ACCELERATION: f32 = 100.0;
const SHIP_ROTATION_SPEED: f32 = 3.0;
const SHIP_RADIUS: f32 = 10.0;
const SHIP_MASS: f32 = 100.0;
const SHIP_MAX_HEALTH: f32 = 100.0;
const COLLISION_DAMAGE_THRESHOLD: f32 = 10.0;

pub struct Ship {
    pub pos: Vec2,
    pub vel: Vec2,
    pub orientation: f32,
    pub health: f32,
}

impl Ship {
    pub fn new(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            orientation: 0.0,
            health: SHIP_MAX_HEALTH,
        }
    }

    pub fn radius(&self) -> f32 {
        SHIP_RADIUS
    }

    pub fn mass(&self) -> f32 {
        SHIP_MASS
    }

    pub fn update(&mut self, asteroids: &mut Vec<Asteroid>, dt: f32) {
        // Calculate gravitational forces from asteroids
        let mut acc = vec2(0.0, 0.0);
        let mut collision_index = None;

        for (i, asteroid) in asteroids.iter().enumerate() {
            let direction = asteroid.pos() - self.pos;
            let distance = direction.length();

            // Check for collision
            if distance <= (self.radius() + asteroid.radius()) {
                collision_index = Some(i);
                break;
            }

            // Same force formula as asteroids: F = mass / distance
            let force_magnitude = asteroid.size() / distance;
            let force = direction.normalize() * force_magnitude;
            acc += force;
        }

        // Handle collision if one occurred
        if let Some(index) = collision_index {
            let asteroid = &asteroids[index];
            let relative_vel = (self.vel - asteroid.vel()).length();

            // Apply damage if relative velocity is high
            if relative_vel > COLLISION_DAMAGE_THRESHOLD {
                let surplus = relative_vel - COLLISION_DAMAGE_THRESHOLD;
                self.health -= surplus * surplus;
            }

            // Inelastic collision: conserve momentum
            let total_mass = self.mass() + asteroid.size();
            let new_vel = (self.vel * self.mass() + asteroid.vel() * asteroid.size()) / total_mass;
            self.vel = new_vel;

            // Separate ship and asteroid to prevent overlap
            let direction = asteroid.pos() - self.pos;
            let distance = direction.length();
            if distance > 0.0 {
                let overlap = (self.radius() + asteroid.radius()) - distance;
                let separation = direction.normalize() * overlap * 0.5;
                self.pos -= separation;
            }
        }

        // Update velocity and position
        self.vel += acc * dt;
        self.pos += self.vel * dt;
    }

    pub fn draw(&self, fb: &mut crate::framebuffer::FrameBuffer, sprite: &image::RgbaImage) {
        let scale = 1.0;
        fb.draw_sprite(sprite, self.pos, scale, self.orientation);
    }

    pub fn apply_control(&mut self, forward: f32, strafe: f32, rotate: f32, dt: f32) {
        // Rotation
        self.orientation += rotate * SHIP_ROTATION_SPEED * dt;

        // Calculate acceleration in ship's local frame
        let cos_angle = self.orientation.cos();
        let sin_angle = self.orientation.sin();

        // Forward/backward: along ship's orientation
        let forward_acc = vec2(sin_angle, -cos_angle) * forward * SHIP_ACCELERATION;

        // Strafe left/right: perpendicular to ship's orientation
        let strafe_acc = vec2(cos_angle, sin_angle) * strafe * SHIP_ACCELERATION;

        // Apply acceleration
        self.vel += (forward_acc + strafe_acc) * dt;
    }
}
