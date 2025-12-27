use std::f32::consts::PI;
use crate::vector::Vector;

#[derive(Default)]
pub struct Asteroid {
    pos: Vector,
    vel: Vector,
    size: f32,
}

impl Asteroid {
    pub fn new(pos: Vector, vel: Vector, size: f32) -> Self {
        Self {
            pos, vel, size
        }
    }
    pub fn pos(&self) -> Vector {
        self.pos
    }
    pub fn radius(self) -> f32 {
        self.size.sqrt() / PI
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

