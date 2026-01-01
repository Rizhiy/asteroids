use crate::objects::Asteroid;
use glam::{Vec2, vec2};
use std::collections::HashSet;

const STATS_UPDATE_RATE: f32 = 5.0;

pub struct WorldState {
    pub asteroids: Vec<Asteroid>,
    pub world_time: f32,
    tick_rate: f32,
    cleanup_threshold_multiplier: f32,
    update_count: u32,
    last_ups_time: std::time::Instant,
    updates_per_second: f32,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            asteroids: Vec::new(),
            world_time: 0.0,
            tick_rate: 100.0,
            cleanup_threshold_multiplier: 10.0,
            update_count: 0,
            last_ups_time: std::time::Instant::now(),
            updates_per_second: 0.0,
        }
    }
}

impl WorldState {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn update(&mut self, delta_time: f32) -> f32 {
        let tick_duration = 1.0 / self.tick_rate;
        let mut delta = delta_time;

        while delta > tick_duration {
            let new_asteroids: Vec<Asteroid> = self
                .asteroids
                .iter()
                .map(|asteroid| {
                    let mut a = *asteroid;
                    a.update(&self.asteroids, tick_duration);
                    a
                })
                .collect();
            self.asteroids = new_asteroids;

            self.check_collisions();
            self.cleanup_distant_asteroids();

            self.world_time += tick_duration;
            delta -= tick_duration;

            self.update_count += 1;
            let update_interval = 1.0 / STATS_UPDATE_RATE;
            if self.last_ups_time.elapsed().as_secs_f32() >= update_interval {
                self.updates_per_second =
                    self.update_count as f32 / self.last_ups_time.elapsed().as_secs_f32();
                self.update_count = 0;
                self.last_ups_time = std::time::Instant::now();
            }
        }

        delta_time - delta
    }

    fn check_collisions(&mut self) {
        let mut to_remove = HashSet::new();
        let mut to_add = Vec::new();

        for i in 0..self.asteroids.len() {
            for j in (i + 1)..self.asteroids.len() {
                if to_remove.contains(&i) || to_remove.contains(&j) {
                    continue;
                }

                let a1 = &self.asteroids[i];
                let a2 = &self.asteroids[j];

                if a1.collides_with(a2) {
                    let merged = a1.merge_with(a2);
                    to_add.push(merged);
                    to_remove.insert(i);
                    to_remove.insert(j);
                }
            }
        }

        let mut idx = 0;
        self.asteroids.retain(|_| {
            let should_keep = !to_remove.contains(&idx);
            idx += 1;
            should_keep
        });

        self.asteroids.extend(to_add);
    }

    pub fn spawn_asteroid(&mut self, pos: Vec2, vel: Vec2, size: f32) {
        self.asteroids.push(Asteroid::new(pos, vel, size));
    }

    pub fn calculate_center_of_mass(&self, weighted: bool) -> Vec2 {
        if self.asteroids.is_empty() {
            return vec2(0.0, 0.0);
        }

        let mut total_mass = 0.0;
        let mut weighted_pos = vec2(0.0, 0.0);

        for asteroid in &self.asteroids {
            let mass = if weighted { asteroid.size() } else { 1.0 };
            total_mass += mass;
            weighted_pos = weighted_pos + asteroid.pos() * mass;
        }

        weighted_pos / total_mass
    }

    fn calculate_mass_std(&self, center: Vec2, weighted: bool) -> f32 {
        if self.asteroids.is_empty() {
            return 0.0;
        }

        let mut total_mass = 0.0;
        let mut weighted_variance = 0.0;

        for asteroid in &self.asteroids {
            let mass = if weighted { asteroid.size() } else { 1.0 };
            let distance = (asteroid.pos() - center).length();
            total_mass += mass;
            weighted_variance += mass * distance * distance;
        }

        if total_mass > 0.0 {
            (weighted_variance / total_mass).sqrt()
        } else {
            0.0
        }
    }

    fn cleanup_distant_asteroids(&mut self) {
        let center = self.calculate_center_of_mass(true);
        let std_dev = self.calculate_mass_std(center, false);
        let threshold = std_dev * self.cleanup_threshold_multiplier;

        self.asteroids
            .retain(|asteroid| (asteroid.pos() - center).length() <= threshold);
    }

    pub fn updates_per_second(&self) -> f32 {
        self.updates_per_second
    }

    pub fn tick_rate(&self) -> f32 {
        self.tick_rate
    }

    pub fn actual_speed(&self) -> f32 {
        self.updates_per_second() / self.tick_rate()
    }
}
