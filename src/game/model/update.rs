use super::*;

impl Model {
    pub fn update(&mut self, delta_time: f32) {
        if self.player.health.is_alive() && self.entities.len() == 0 && self.spawners.len() == 0 {
            self.next_wave();
        }

        self.update_spawners(delta_time);
        self.particles(delta_time);
    }

    fn update_spawners(&mut self, delta_time: f32) {
        let mut remove_spawners = Vec::new();
        for (index, spawner) in self.spawners.iter_mut().enumerate() {
            spawner.time_left -= delta_time;
            if spawner.time_left <= 0.0 {
                remove_spawners.push(index);
            }
        }
        remove_spawners.reverse();
        for spawner_index in remove_spawners {
            let spawner = self.spawners.remove(spawner_index);
            self.spawn_group(spawner.position, spawner.spawn_group);
        }
    }

    fn particles(&mut self, delta_time: f32) {
        for particle in &mut self.particles {
            particle.rigidbody.movement(delta_time);
            particle.rigidbody.bounce_bounds(&self.bounds);
            particle.rigidbody.drag(delta_time);
            particle.lifetime.change(-delta_time);
            particle.color.a = particle.lifetime.hp_frac() * 0.5;
        }
        self.particles
            .retain(|particle| particle.lifetime.is_alive());
    }

    pub fn fixed_update(&mut self, delta_time: f32) {
        let mut commands = Commands::new();

        self.attack(delta_time, &mut commands);
        self.area_effects(delta_time);
        self.move_player(delta_time);
        self.move_enemies(delta_time);
        self.collide(&mut commands);
        self.check_dead(delta_time);

        self.perform_commands(commands);
    }

    fn attack(&mut self, delta_time: f32, commands: &mut Commands) {
        for entity in &mut self.entities {
            // Update
            match &mut entity.entity_type {
                EntityType::Player { .. } => (),
                EntityType::Enemy { enemy_type } => match enemy_type {
                    EnemyType::Attacker { attack } => {
                        attack.attack_time.change(-delta_time);
                        match &mut attack.attack_type {
                            AttackType::Shoot { target_pos, .. } => {
                                *target_pos = self.player.body.position;
                            }
                            _ => (),
                        }
                    }
                    EnemyType::Projectile { lifetime, .. } => {
                        lifetime.change(-delta_time);
                        if !lifetime.is_alive() {
                            entity.health.kill();
                        }
                    }
                    _ => (),
                },
            }
            // Attack
            entity.attack(commands);
            // Reset
            entity.reset_attacks();
        }
    }

    fn area_effects(&mut self, delta_time: f32) {
        for area_effect in &mut self.area_effects {
            area_effect.lifetime.change(-delta_time);

            if self.player.health.is_alive() {
                let distance = (area_effect.position - self.player.body.position).length();
                if distance <= self.player.body.collider.radius + area_effect.radius {
                    match &area_effect.effect {
                        Effect::Heal { heal } => {
                            self.player.health.change(*heal * delta_time);
                        }
                    }
                }
            }
        }
        self.area_effects
            .retain(|area_effect| area_effect.lifetime.is_alive());
    }

    fn move_player(&mut self, delta_time: f32) {
        // Move
        self.player.body.movement(delta_time);
        self.player.head.movement(delta_time);

        if self.player.health.is_alive() {
            // Calculate head target velocity
            let direction = self.player.head.position - self.player.body.position;
            let target = self.player.head_target - self.player.body.position;
            let angle = direction.angle_between(target).abs();
            let speed = angle.min(0.2) / 0.2;
            let direction = vec2(direction.y, -direction.x).normalize();
            let signum = direction.dot(target).signum();
            let direction = direction * signum * speed;
            self.player.target_head_velocity = direction * HEAD_SPEED + self.player.body.velocity;

            // Accelerate towards target velocity
            let target_change = self.player.target_body_velocity - self.player.body.velocity;
            self.player.body.velocity += target_change * BODY_ACCELERATION * delta_time;

            let target_change = self.player.target_head_velocity - self.player.head.velocity;
            self.player.head.velocity += target_change * HEAD_ACCELERATION * delta_time;
        }

        // Clamp distance between body and head
        let offset = self.player.head.position - self.player.body.position;
        let distance = offset.length() - self.player.chain_length;
        self.player.head.position -= offset.normalize_or_zero() * distance;
    }

    fn move_enemies(&mut self, delta_time: f32) {
        for entity in &mut self.entities {
            entity.rigidbody.movement(delta_time);

            if entity.rigidbody.velocity.length() > entity.movement_speed {
                entity.rigidbody.drag(delta_time);
            }
            match &entity.entity_type {
                EntityType::Enemy { enemy_type } => match enemy_type {
                    EnemyType::Crawler | EnemyType::Attacker { .. } => {
                        let target_direction =
                            self.player.body.position - entity.rigidbody.position;
                        let target_velocity = target_direction.normalize() * entity.movement_speed;
                        entity.rigidbody.velocity +=
                            (target_velocity - entity.rigidbody.velocity) * delta_time;
                    }
                    _ => (),
                },
                _ => (),
            }
        }
    }

    fn collide(&mut self, commands: &mut Commands) {
        // Collide bounds
        if self.player.body.bounce_bounds(&self.bounds) {
            self.events.push(Event::Sound {
                sound: EventSound::Bounce,
            });
        }
        self.player.head.bounce_bounds(&self.bounds);
        for entity in &mut self.entities {
            if entity.rigidbody.bounce_bounds(&self.bounds) {
                if let EntityType::Enemy {
                    enemy_type: EnemyType::Projectile { lifetime },
                } = &mut entity.entity_type
                {
                    lifetime.kill();
                }

                self.events.push(Event::Sound {
                    sound: EventSound::Bounce,
                });
            }
        }

        // Collide player body
        for entity in &mut self.entities {
            if !entity.is_alive() {
                continue;
            }

            if let Some(collision) = entity.rigidbody.collide(&self.player.body) {
                entity.rigidbody.position += collision.normal * collision.penetration;
                let relative_velocity = self.player.body.velocity - entity.rigidbody.velocity;
                let hit_strength = collision.normal.dot(relative_velocity).abs();
                entity.rigidbody.velocity +=
                    BODY_HIT_SPEED * collision.normal * self.player.body.mass
                        / entity.rigidbody.mass;
                self.player.body.velocity -=
                    BODY_IMPACT * collision.normal * entity.rigidbody.mass / self.player.body.mass;

                let contact = self.player.body.position + collision.normal * collision.penetration;
                let player_alive = self.player.health.is_alive();
                self.player.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength * 5.0, PLAYER_COLOR);
                entity.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength, entity.color);
                self.events.push(Event::Sound {
                    sound: EventSound::BodyHit,
                });
                if player_alive && !self.player.health.is_alive() {
                    self.events.push(Event::Sound {
                        sound: EventSound::Death,
                    })
                }
            }
        }

        // Collide player head
        for entity in &mut self.entities {
            if !entity.is_alive() {
                continue;
            }

            if let Some(collision) = entity.rigidbody.collide(&self.player.head) {
                entity.rigidbody.position += collision.normal * collision.penetration;
                let relative_velocity = self.player.head.velocity - entity.rigidbody.velocity;
                let hit_strength = collision.normal.dot(relative_velocity).abs();
                entity.rigidbody.velocity +=
                    hit_strength * collision.normal * self.player.head.mass / entity.rigidbody.mass;
                self.player.head.velocity -=
                    hit_strength * collision.normal * entity.rigidbody.mass / self.player.body.mass;

                let contact = self.player.head.position + collision.normal * collision.penetration;
                entity.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength, entity.color);
                self.events.push(Event::Sound {
                    sound: EventSound::HeadHit,
                });
            }
        }
    }

    fn check_dead(&mut self, delta_time: f32) {
        let mut dead_enemies = Vec::new();
        for (index, entity) in self.entities.iter_mut().enumerate() {
            if entity.destroy {
                dead_enemies.push(index);
            } else if !entity.is_alive() {
                match &mut entity.entity_type {
                    EntityType::Enemy { enemy_type } => match enemy_type {
                        EnemyType::Corpse { lifetime } => {
                            lifetime.change(-delta_time);
                            if !lifetime.is_alive() {
                                dead_enemies.push(index);
                            }
                            entity.color.a = lifetime.hp_frac() * 0.5;
                        }
                        EnemyType::Attacker { attack } if !attack.attack_time.is_alive() => {
                            match attack.attack_type {
                                AttackType::Bomb { .. } => {
                                    dead_enemies.push(index);
                                }
                                _ => (),
                            }
                        }
                        _ => {
                            *enemy_type = EnemyType::Corpse {
                                lifetime: Health::new(CORPSE_LIFETIME),
                            }
                        }
                    },
                    _ => (),
                }
            }
        }
        dead_enemies.reverse();
        for dead_index in dead_enemies {
            self.entities.remove(dead_index);
        }
    }
}
