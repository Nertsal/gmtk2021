use super::*;

#[derive(Debug, Clone)]
pub struct Collider {
    pub radius: f32,
}

impl Collider {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

#[derive(Debug, Clone)]
pub struct Collision {
    pub normal: vec2<f32>,
    pub penetration: f32,
}

#[derive(Debug, Clone)]
pub struct HitInfo {
    pub contact: vec2<f32>,
    pub hit_self: f32,
    pub hit_other: f32,
}
