use crate::objects::Asteroid;
use glam::{Vec2, vec2};

const RCS_ACCELERATION: f32 = 10.0;
const MAIN_ENGINE_ACCELERATION: f32 = RCS_ACCELERATION * 100.0;
const ENGINE_POWER_CHANGE_RATE: f32 = 0.5;
const SHIP_ROTATION_SPEED: f32 = 3.0;
const SHIP_RADIUS: f32 = 10.0;
const SHIP_MASS: f32 = 100.0;
const SHIP_MAX_HEALTH: f32 = 10000.0;
const COLLISION_DAMAGE_THRESHOLD: f32 = 10.0;
const RESTITUTION: f32 = 0.5;

pub struct Ship {
    pub pos: Vec2,
    pub vel: Vec2,
    pub orientation: f32,
    pub health: f32,
    pub engine_power: f32,
}

impl Ship {
    pub fn new(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            orientation: 0.0,
            health: SHIP_MAX_HEALTH,
            engine_power: 0.0,
        }
    }

    pub fn radius(&self) -> f32 {
        SHIP_RADIUS
    }

    pub fn mass(&self) -> f32 {
        SHIP_MASS
    }

    pub fn update(&mut self, asteroids: &mut Vec<Asteroid>, dt: f32) {
        // Calculate gravitational forces from asteroids and collect collisions
        let mut acc = vec2(0.0, 0.0);
        let mut collision_data = Vec::new();

        for (i, asteroid) in asteroids.iter_mut().enumerate() {
            let direction = asteroid.pos() - self.pos;
            let distance = direction.length();

            // Check for collision
            if distance <= (self.radius() + asteroid.radius()) {
                collision_data.push((i, direction, distance));
            } else {
                // Apply asteroid's gravity on ship
                let force_magnitude = asteroid.size() / distance;
                let force = direction.normalize() * force_magnitude;
                acc += force;

                // Apply ship's gravity on asteroid
                let ship_force_magnitude = self.mass() / distance;
                let ship_force = -direction.normalize() * ship_force_magnitude;
                asteroid.set_vel(asteroid.vel() + ship_force * dt);
            }
        }

        // Accumulate velocity changes from all collisions
        let mut ship_vel_delta = Vec2::ZERO;

        for (index, direction, distance) in collision_data {
            let asteroid = &mut asteroids[index];

            if distance > 0.0 {
                let overlap = (self.radius() + asteroid.radius()) - distance;
                let normal = direction.normalize();

                let ship_mass = self.mass();
                let asteroid_mass = asteroid.size();
                let total_mass = ship_mass + asteroid_mass;

                // Calculate relative velocity
                let relative_vel = asteroid.vel() - self.vel;
                let relative_vel_magnitude = relative_vel.length();
                let rel_vel_along_normal = relative_vel.dot(normal);

                // Apply damage if relative velocity is high
                if relative_vel_magnitude > COLLISION_DAMAGE_THRESHOLD {
                    let surplus = relative_vel_magnitude - COLLISION_DAMAGE_THRESHOLD;
                    self.health -= surplus * surplus * asteroid_mass / total_mass;
                }

                // Separate objects based on their velocities and masses
                if overlap > 0.0 {
                    let ship_mass_ratio = ship_mass / total_mass;
                    let asteroid_mass_ratio = asteroid_mass / total_mass;

                    // Separate in proportion to masses
                    self.pos -= normal * overlap * asteroid_mass_ratio;
                    asteroid.set_pos(asteroid.pos() + normal * overlap * ship_mass_ratio);
                }

                // Impulse magnitude using reduced mass formula
                let impulse_magnitude =
                    -(1.0 + RESTITUTION) * rel_vel_along_normal * (ship_mass * asteroid_mass)
                        / total_mass;
                let impulse = normal * impulse_magnitude;

                // Apply velocity changes (impulse / mass = velocity change)
                ship_vel_delta -= impulse / ship_mass;
                asteroid.set_vel(asteroid.vel() + impulse / asteroid_mass);
            }
        }

        // Apply accumulated velocity changes
        self.vel += ship_vel_delta;

        // Update velocity and position
        self.vel += acc * dt;
        self.pos += self.vel * dt;
    }

    pub fn draw(&self, fb: &mut crate::framebuffer::FrameBuffer, sprite: &image::RgbaImage) {
        // Scale sprite to match ship radius (diameter = 2 * radius)
        let sprite_world_size = self.radius() * 2.0;
        let scale = sprite_world_size / sprite.width() as f32;
        fb.draw_sprite(sprite, self.pos, scale, self.orientation);
    }

    pub fn draw_engine_indicator(&self, fb: &mut crate::framebuffer::FrameBuffer) {
        use crate::color::Color;
        use glam::vec2;

        let indicator_x = 10;
        let indicator_width = 20;
        let indicator_height = indicator_width * 5;
        let indicator_y = fb.height() as i32 - indicator_height - 10;

        // Draw outer rectangle border
        let border_color = Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        };

        // Draw rectangle using lines
        let top_left = vec2(indicator_x as f32, indicator_y as f32);
        let top_right = vec2((indicator_x + indicator_width) as f32, indicator_y as f32);
        let bottom_left = vec2(indicator_x as f32, (indicator_y + indicator_height) as f32);
        let bottom_right = vec2(
            (indicator_x + indicator_width) as f32,
            (indicator_y + indicator_height) as f32,
        );

        fb.draw_screen_line(top_left, top_right, border_color);
        fb.draw_screen_line(top_right, bottom_right, border_color);
        fb.draw_screen_line(bottom_right, bottom_left, border_color);
        fb.draw_screen_line(bottom_left, top_left, border_color);

        // Draw horizontal power level line
        let power_y = indicator_y + ((1.0 - self.engine_power) * indicator_height as f32) as i32;
        let power_color = Color {
            r: 0,
            g: 200,
            b: 255,
            a: 255,
        };

        // Draw thicker line (3 lines for thickness)
        let power_left = vec2((indicator_x + 1) as f32, power_y as f32);
        let power_right = vec2((indicator_x + indicator_width - 1) as f32, power_y as f32);

        for dy in -1..=1 {
            fb.draw_screen_line(
                vec2(power_left.x, power_left.y + dy as f32),
                vec2(power_right.x, power_right.y + dy as f32),
                power_color,
            );
        }

        // Draw label
        let engine_text = format!("{}%", (self.engine_power * 100.0) as u32);
        fb.draw_text(
            &engine_text,
            vec2(indicator_x as f32, (indicator_y - 20) as f32),
            16.0,
            Color::WHITE,
        );

        // Draw orientation arrow to the right of the indicator
        let arrow_center_x = indicator_x + indicator_width + 40;
        let arrow_center_y = indicator_y + indicator_height / 2;
        let arrow_size = 15;

        let arrow_color = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };

        // Draw circle background
        let bg_color = Color {
            r: 50,
            g: 50,
            b: 50,
            a: 200,
        };
        fb.draw_circle(
            vec2(arrow_center_x as f32, arrow_center_y as f32),
            arrow_size as f32,
            bg_color,
        );

        // Draw arrow pointing in ship's orientation
        let cos_angle = self.orientation.cos();
        let sin_angle = self.orientation.sin();
        let arrow_length = 10.0;

        // Arrow tip
        let tip = vec2(
            arrow_center_x as f32 + sin_angle * arrow_length,
            arrow_center_y as f32 - cos_angle * arrow_length,
        );

        // Arrow base points
        let base_width = 3.0;
        let base1 = vec2(
            arrow_center_x as f32 + cos_angle * base_width - sin_angle * arrow_length * 0.3,
            arrow_center_y as f32 + sin_angle * base_width + cos_angle * arrow_length * 0.3,
        );
        let base2 = vec2(
            arrow_center_x as f32 - cos_angle * base_width - sin_angle * arrow_length * 0.3,
            arrow_center_y as f32 - sin_angle * base_width + cos_angle * arrow_length * 0.3,
        );

        // Draw arrow lines
        fb.draw_screen_line(base1, tip, arrow_color);
        fb.draw_screen_line(base2, tip, arrow_color);
        fb.draw_screen_line(base1, base2, arrow_color);
    }

    pub fn draw_health_bar(&self, fb: &mut crate::framebuffer::FrameBuffer) {
        use crate::color::Color;
        use glam::vec2;

        let bar_width = 20;
        let bar_height = 100;
        let bar_x = fb.width() as i32 - bar_width - 10;
        let bar_y = fb.height() as i32 - bar_height - 10;

        // Draw border
        let border_color = Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        };

        let top_left = vec2(bar_x as f32, bar_y as f32);
        let top_right = vec2((bar_x + bar_width) as f32, bar_y as f32);
        let bottom_left = vec2(bar_x as f32, (bar_y + bar_height) as f32);
        let bottom_right = vec2((bar_x + bar_width) as f32, (bar_y + bar_height) as f32);

        fb.draw_screen_line(top_left, top_right, border_color);
        fb.draw_screen_line(top_right, bottom_right, border_color);
        fb.draw_screen_line(bottom_right, bottom_left, border_color);
        fb.draw_screen_line(bottom_left, top_left, border_color);

        // Draw filled health bar
        let health_ratio = (self.health / SHIP_MAX_HEALTH).clamp(0.0, 1.0);
        let filled_height = bar_height as f32 * health_ratio;

        if filled_height > 0.0 {
            let health_color = Color {
                r: 0,
                g: 150,
                b: 255,
                a: 255,
            };

            // Fill from bottom up
            let fill_top_left = vec2(
                (bar_x + 1) as f32,
                (bar_y + bar_height) as f32 - filled_height,
            );

            fb.draw_screen_rectangle(
                fill_top_left,
                (bar_width - 2) as f32,
                filled_height,
                health_color,
            );
        }

        // Draw health percentage label
        let health_text = format!("{}%", (health_ratio * 100.0) as u32);
        fb.draw_text(
            &health_text,
            vec2((bar_x - 10) as f32, (bar_y - 20) as f32),
            16.0,
            Color::WHITE,
        );
    }

    pub fn apply_control(
        &mut self,
        rcs_forward: f32,
        rcs_strafe: f32,
        rotate: f32,
        engine_increase: bool,
        engine_decrease: bool,
        dt: f32,
    ) {
        // Rotation
        self.orientation += rotate * SHIP_ROTATION_SPEED * dt;

        // Update engine power
        if engine_increase {
            self.engine_power += ENGINE_POWER_CHANGE_RATE * dt;
        }
        if engine_decrease {
            self.engine_power -= ENGINE_POWER_CHANGE_RATE * dt;
        }
        self.engine_power = self.engine_power.clamp(0.0, 1.0);

        // Calculate acceleration in ship's local frame
        let cos_angle = self.orientation.cos();
        let sin_angle = self.orientation.sin();

        // RCS thrusters (WASD)
        let rcs_forward_acc = vec2(sin_angle, -cos_angle) * rcs_forward * RCS_ACCELERATION;
        let rcs_strafe_acc = vec2(cos_angle, sin_angle) * rcs_strafe * RCS_ACCELERATION;

        // Main engine (always forward)
        let main_engine_acc =
            vec2(sin_angle, -cos_angle) * self.engine_power * MAIN_ENGINE_ACCELERATION;

        // Apply acceleration
        self.vel += (rcs_forward_acc + rcs_strafe_acc + main_engine_acc) * dt;
    }
}
