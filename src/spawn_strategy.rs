use crate::framebuffer::FrameBuffer;
use crate::objects::Asteroid;
use crate::world::WorldState;
use glam::{vec2, Vec2};

pub trait SpawnStrategy {
    fn spawn(&mut self, world: &mut WorldState, fb: &FrameBuffer);

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
    fn spawn(&mut self, world: &mut WorldState, fb: &FrameBuffer) {
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

        world.asteroids.push(Asteroid::new(pos, vel, size));
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
    fn spawn(&mut self, world: &mut WorldState, fb: &FrameBuffer) {
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

        world.asteroids.push(Asteroid::new(pos, vel, size));
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

pub struct SolarSystemStrategy {
    star_spawned: bool,
    planets: Vec<PlanetData>,
    total_moons: usize,
}

#[derive(Clone)]
struct PlanetData {
    pos: Vec2,
    vel: Vec2,
    radius: f32,
}

impl SolarSystemStrategy {
    pub fn new() -> Self {
        Self {
            star_spawned: false,
            planets: Vec::new(),
            total_moons: 0,
        }
    }

    fn is_complete(&self) -> bool {
        self.star_spawned && self.planets.len() >= 10 && self.total_moons >= 30
    }
}

impl SpawnStrategy for SolarSystemStrategy {
    fn spawn(&mut self, world: &mut WorldState, _fb: &FrameBuffer) {
        const SHIP_RADIUS: f32 = 10.0;
        const STAR_RADIUS_MULTIPLIER: f32 = 125.0;
        const PLANET_RADIUS_MULTIPLIER: f32 = 25.0;
        const MOON_RADIUS_MULTIPLIER: f32 = 5.0;
        const PLANET_ORBIT_MULTIPLIER: f32 = 10.0;
        const MOON_ORBIT_MULTIPLIER: f32 = 3.0;

        if self.is_complete() {
            return;
        }

        // Step 1: Spawn the star
        if !self.star_spawned {
            let star_radius = SHIP_RADIUS * STAR_RADIUS_MULTIPLIER;
            let star_mass = star_radius * star_radius * std::f32::consts::PI;

            // Spawn star far from ship to avoid immediate collision
            let safe_distance = star_radius * 5.0;
            let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;
            let star_pos =
                world.ship.pos + vec2(angle.cos() * safe_distance, angle.sin() * safe_distance);

            // Set ship velocity to orbit the star
            // For F = M / r, orbital velocity is v = sqrt(M)
            let orbital_speed = star_mass.sqrt();

            // Ship orbits perpendicular to the radius vector (from ship to star)
            // Since angle points from ship to star, perpendicular is (sin, -cos) for counter-clockwise
            world.ship.vel = vec2(angle.sin() * orbital_speed, -angle.cos() * orbital_speed);

            self.star_spawned = true;
            world
                .asteroids
                .push(Asteroid::new(star_pos, vec2(0.0, 0.0), star_mass));
            return;
        }

        // Get star position (it's the first asteroid spawned by this strategy)
        // We assume it's the most massive asteroid
        let star = world
            .asteroids
            .iter()
            .max_by(|a, b| a.size().partial_cmp(&b.size()).unwrap());

        if star.is_none() {
            return;
        }
        let star = star.unwrap();
        let star_pos = star.pos();
        let star_mass = star.size();
        let star_radius = (star_mass / std::f32::consts::PI).sqrt();

        // Step 2 & 3: Spawn planets or moons
        if self.planets.len() < 10 {
            // Or 50/50 chance if we already have at least one planet
            let should_spawn_planet = self.planets.is_empty() || fastrand::bool();

            if should_spawn_planet {
                let planet_radius = SHIP_RADIUS * PLANET_RADIUS_MULTIPLIER;
                let planet_mass = planet_radius * planet_radius * std::f32::consts::PI;

                let orbit_radius = star_radius * PLANET_ORBIT_MULTIPLIER;
                let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;
                let planet_pos =
                    star_pos + vec2(angle.cos() * orbit_radius, angle.sin() * orbit_radius);

                // Calculate orbital velocity for F = M / r
                // For circular orbit: v^2 / r = M / r, so v^2 = M, thus v = sqrt(M)
                let orbital_speed = star_mass.sqrt();
                let planet_vel = vec2(-angle.sin() * orbital_speed, angle.cos() * orbital_speed);

                self.planets.push(PlanetData {
                    pos: planet_pos,
                    vel: planet_vel,
                    radius: planet_radius,
                });

                world
                    .asteroids
                    .push(Asteroid::new(planet_pos, planet_vel, planet_mass));
                return;
            }
        }

        // Spawn a moon if we have planets and haven't reached 30 moons
        if !self.planets.is_empty() && self.total_moons < 30 {
            // Pick a random planet
            let planet_idx = fastrand::usize(0..self.planets.len());
            let planet = self.planets[planet_idx].clone();

            let moon_radius = SHIP_RADIUS * MOON_RADIUS_MULTIPLIER;
            let moon_mass = moon_radius * moon_radius * std::f32::consts::PI;

            let orbit_radius = planet.radius * MOON_ORBIT_MULTIPLIER;
            let angle = fastrand::f32() * 2.0 * std::f32::consts::PI;
            let moon_relative_pos = vec2(angle.cos() * orbit_radius, angle.sin() * orbit_radius);
            let moon_pos = planet.pos + moon_relative_pos;

            // Calculate orbital velocity around planet for F = M / r
            // For circular orbit: v^2 / r = M / r, so v^2 = M, thus v = sqrt(M)
            let planet_mass = planet.radius * planet.radius * std::f32::consts::PI;
            let orbital_speed = planet_mass.sqrt();
            let moon_orbital_vel = vec2(-angle.sin() * orbital_speed, angle.cos() * orbital_speed);

            // Add planet's velocity to moon's orbital velocity
            let moon_vel = planet.vel + moon_orbital_vel;

            self.total_moons += 1;

            world
                .asteroids
                .push(Asteroid::new(moon_pos, moon_vel, moon_mass));
        }
    }

    fn name(&self) -> &str {
        "Solar System"
    }
}
