use super::*;

pub struct Particle {
    pub rigidbody: RigidBody,
    pub color: Color,
    pub lifetime: Health,
}

impl Model {
    pub fn spawn_particles_hit(&mut self, position: vec2<f32>, intensity: f32, color: Color) {
        let mut rng = thread_rng();
        let particles_count = rng.gen_range(1..(intensity / 10.0).min(50.0) as usize);
        for _ in 0..particles_count {
            let direction = Self::get_random_direction();
            let velocity = rng.gen_range(10.0..30.0);
            let velocity = direction * velocity;
            self.particles.push(Particle {
                rigidbody: RigidBody {
                    position,
                    velocity,
                    mass: 1.0,
                    is_kinematic: false,
                    collider: Collider::new(1.0),
                    physics_material: PhysicsMaterial::new(DRAG, BOUNCINESS),
                },
                color,
                lifetime: Health::new(PARTICLE_LIFETIME),
            })
        }
    }

    pub fn get_random_direction() -> vec2<f32> {
        let angle = thread_rng().gen_range(0.0..std::f32::consts::PI * 2.0);
        let (sin, cos) = angle.sin_cos();
        vec2(cos, sin)
    }
}
