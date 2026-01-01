use crate::framebuffer::FrameBuffer;
use crate::objects::Asteroid;
use crate::world::WorldState;
use glam::vec2;

pub trait SpawnStrategy {
    fn spawn(&self, world: &WorldState, fb: &FrameBuffer) -> Vec<Asteroid>;

    fn name(&self) -> &str;
}

pub struct RandomScreenSpaceStrategy {
    pub min_size: f32,
    pub size_alpha: f32,
    pub min_speed: f32,
    pub speed_alpha: f32,
    pub max_size: f32,
    pub max_speed: f32,
}

impl RandomScreenSpaceStrategy {
    pub fn new() -> Self {
        Self {
            min_size: 1.0,
            size_alpha: 0.667,
            min_speed: 1.0,
            speed_alpha: 1.354,
            max_size: 10000.0,
            max_speed: 10000.0,
        }
    }
}

impl SpawnStrategy for RandomScreenSpaceStrategy {
    fn spawn(&self, world: &WorldState, fb: &FrameBuffer) -> Vec<Asteroid> {
        let width = fb.width() as f32 / fb.zoom;
        let height = fb.height() as f32 / fb.zoom;

        let x = fb.camera_pos.x + (fastrand::f32() - 0.5) * width;
        let y = fb.camera_pos.y + (fastrand::f32() - 0.5) * height;
        let pos = vec2(x, y);

        let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;

        let speed = power_law_sample(self.min_speed, self.speed_alpha).min(self.max_speed);
        let random_vel = vec2(angle.cos() * speed, angle.sin() * speed);
        let actual_speed = world.actual_speed();
        let mut vel = random_vel + fb.camera_vel;
        if actual_speed > 0.0 {
            vel /= actual_speed;
        }

        let size = power_law_sample(self.min_size, self.size_alpha).min(self.max_size);

        vec![Asteroid::new(pos, vel, size)]
    }

    fn name(&self) -> &str {
        "Random"
    }
}

pub struct OrbitalDiskStrategy {
    pub min_radius: f32,
    pub max_radius_multiplier: f32,
    pub mean_size: f32,
    pub size_std_dev: f32,
    pub velocity_std_dev: f32,
}

impl OrbitalDiskStrategy {
    pub fn new() -> Self {
        Self {
            min_radius: 0.0,
            max_radius_multiplier: 500.0,
            mean_size: 5.0,
            size_std_dev: 1.0,
            velocity_std_dev: 0.1,
        }
    }
}

impl SpawnStrategy for OrbitalDiskStrategy {
    fn spawn(&self, world: &WorldState, fb: &FrameBuffer) -> Vec<Asteroid> {
        let center = world.calculate_center_of_mass(true);

        // Max radius depends on zoom level (more zoomed out = larger spawn area)
        let max_radius = self.max_radius_multiplier / fb.zoom;

        // Random radius with uniform distribution over circular area
        // Using sqrt to get uniform area distribution (not uniform radius distribution)
        let u = fastrand::f32();
        let radius = self.min_radius + u.sqrt() * (max_radius - self.min_radius);

        // Random angle
        let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;

        // Position on disk
        let pos = center + vec2(angle.cos() * radius, angle.sin() * radius);

        // Calculate orbital velocity for linear force decay: F = M / r
        // For circular orbit: centripetal acceleration = v^2 / r
        // Force per unit mass: a = M / r
        // Therefore: v^2 / r = M / r, so v^2 = M
        // Thus: v = sqrt(M)
        let central_mass = world.asteroids.iter().map(|a| a.size()).sum::<f32>();
        let orbital_speed = central_mass.sqrt();

        // Add velocity perturbation using normal distribution
        let velocity_perturbation = normal_sample(1.0, self.velocity_std_dev);
        let perturbed_speed = orbital_speed * velocity_perturbation;

        // Velocity perpendicular to radius (tangential)
        let vel = vec2(
            -angle.sin() * perturbed_speed,
            angle.cos() * perturbed_speed,
        );

        // Size from normal distribution
        let size = normal_sample(self.mean_size, self.size_std_dev).max(0.1);

        vec![Asteroid::new(pos, vel, size)]
    }

    fn name(&self) -> &str {
        "Orbital"
    }
}

fn power_law_sample(min_value: f32, alpha: f32) -> f32 {
    let u = fastrand::f32();
    min_value * (1.0 - u).powf(-1.0 / alpha)
}

fn normal_sample(mean: f32, std_dev: f32) -> f32 {
    // Box-Muller transform to generate normal distribution
    let u1 = fastrand::f32();
    let u2 = fastrand::f32();
    let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
    mean + std_dev * z0
}
