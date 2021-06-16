use super::*;

impl Model {
    pub fn update(&mut self, delta_time: f32) {
        if self.player.entity.is_alive() && self.enemies.len() == 0 && self.spawners.len() == 0 {
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
        for enemy in &mut self.enemies {
            // Update
            match &mut enemy.enemy_type {
                EnemyType::Attacker { attack } => {
                    attack.attack_time.change(-delta_time);
                    match &mut attack.attack_type {
                        AttackType::Shoot { target_pos, .. } => {
                            *target_pos = self.player.entity.rigidbody.position;
                        }
                        _ => (),
                    }
                    attack.perform(&mut enemy.entity, commands);
                }
                EnemyType::Projectile { lifetime, .. } => {
                    lifetime.change(-delta_time);
                    if !lifetime.is_alive() {
                        enemy.entity.health.kill();
                    }
                }
                _ => (),
            }
        }
    }

    fn area_effects(&mut self, delta_time: f32) {
        for area_effect in &mut self.area_effects {
            area_effect.lifetime.change(-delta_time);

            if self.player.entity.is_alive() {
                let distance =
                    (area_effect.position - self.player.entity.rigidbody.position).length();
                if distance <= self.player.entity.rigidbody.collider.radius + area_effect.radius {
                    match &area_effect.effect {
                        Effect::Heal { heal } => {
                            self.player.entity.health.change(*heal * delta_time);
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
        self.player.entity.rigidbody.movement(delta_time);
        self.player.head.movement(delta_time);

        if self.player.entity.is_alive() {
            // Calculate head target velocity
            let direction = self.player.head.position - self.player.entity.rigidbody.position;
            let target = self.player.head_target - self.player.entity.rigidbody.position;
            let angle = direction.angle_between(target).abs();
            let speed = angle.min(0.2) / 0.2;
            let direction = vec2(direction.y, -direction.x).normalize();
            let signum = direction.dot(target).signum();
            let direction = direction * signum * speed;
            self.player.target_head_velocity =
                direction * HEAD_SPEED + self.player.entity.rigidbody.velocity;

            // Accelerate towards target velocity
            let target_change =
                self.player.target_body_velocity - self.player.entity.rigidbody.velocity;
            self.player.entity.rigidbody.velocity += target_change * BODY_ACCELERATION * delta_time;

            let target_change = self.player.target_head_velocity - self.player.head.velocity;
            self.player.head.velocity += target_change * HEAD_ACCELERATION * delta_time;
        }

        // Clamp distance between body and head
        let offset = self.player.head.position - self.player.entity.rigidbody.position;
        let distance = offset.length() - self.player.chain_length;
        self.player.head.position -= offset.normalize_or_zero() * distance;
    }

    fn move_enemies(&mut self, delta_time: f32) {
        for enemy in &mut self.enemies {
            enemy.entity.rigidbody.movement(delta_time);

            if enemy.entity.rigidbody.velocity.length() > enemy.entity.movement_speed {
                enemy.entity.rigidbody.drag(delta_time);
            }
            match &enemy.enemy_type {
                EnemyType::Crawler | EnemyType::Attacker { .. } => {
                    let target_direction =
                        self.player.entity.rigidbody.position - enemy.entity.rigidbody.position;
                    let target_velocity =
                        target_direction.normalize() * enemy.entity.movement_speed;
                    enemy.entity.rigidbody.velocity +=
                        (target_velocity - enemy.entity.rigidbody.velocity) * delta_time;
                }
                _ => (),
            }
        }
    }

    fn collide(&mut self, commands: &mut Commands) {
        // Collide bounds
        if self.player.entity.rigidbody.bounce_bounds(&self.bounds) {
            self.events.push(Event::Sound {
                sound: EventSound::Bounce,
            });
        }
        self.player.head.bounce_bounds(&self.bounds);
        for enemy in &mut self.enemies {
            if enemy.entity.rigidbody.bounce_bounds(&self.bounds) {
                if let EnemyType::Projectile { lifetime } = &mut enemy.enemy_type {
                    lifetime.kill();
                }

                self.events.push(Event::Sound {
                    sound: EventSound::Bounce,
                });
            }
        }

        // Collide player body
        for enemy in &mut self.enemies {
            if !enemy.entity.is_alive() {
                continue;
            }

            if let Some(collision) = enemy
                .entity
                .rigidbody
                .collide(&self.player.entity.rigidbody)
            {
                enemy.entity.rigidbody.position += collision.normal * collision.penetration;
                let relative_velocity =
                    self.player.entity.rigidbody.velocity - enemy.entity.rigidbody.velocity;
                let hit_strength = collision.normal.dot(relative_velocity).abs();
                enemy.entity.rigidbody.velocity +=
                    BODY_HIT_SPEED * collision.normal * self.player.entity.rigidbody.mass
                        / enemy.entity.rigidbody.mass;
                self.player.entity.rigidbody.velocity -=
                    BODY_IMPACT * collision.normal * enemy.entity.rigidbody.mass
                        / self.player.entity.rigidbody.mass;

                let contact = self.player.entity.rigidbody.position
                    + collision.normal * collision.penetration;
                let player_alive = self.player.entity.is_alive();
                self.player.entity.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength * 5.0, PLAYER_COLOR);
                enemy.entity.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength, enemy.entity.color);
                self.events.push(Event::Sound {
                    sound: EventSound::BodyHit,
                });
                if player_alive && !self.player.entity.is_alive() {
                    self.events.push(Event::Sound {
                        sound: EventSound::Death,
                    })
                }
            }
        }

        // Collide player head
        for enemy in &mut self.enemies {
            if !enemy.entity.is_alive() {
                continue;
            }

            if let Some(collision) = enemy.entity.rigidbody.collide(&self.player.head) {
                enemy.entity.rigidbody.position += collision.normal * collision.penetration;
                let relative_velocity = self.player.head.velocity - enemy.entity.rigidbody.velocity;
                let hit_strength = collision.normal.dot(relative_velocity).abs();
                enemy.entity.rigidbody.velocity +=
                    hit_strength * collision.normal * self.player.head.mass
                        / enemy.entity.rigidbody.mass;
                self.player.head.velocity -=
                    hit_strength * collision.normal * enemy.entity.rigidbody.mass
                        / self.player.entity.rigidbody.mass;

                let contact = self.player.head.position + collision.normal * collision.penetration;
                enemy.entity.health.change(-hit_strength);
                commands.spawn_particles(contact, hit_strength, enemy.entity.color);
                self.events.push(Event::Sound {
                    sound: EventSound::HeadHit,
                });
            }
        }
    }

    fn check_dead(&mut self, delta_time: f32) {
        let mut dead_enemies = Vec::new();
        for (index, enemy) in self.enemies.iter_mut().enumerate() {
            if enemy.entity.destroy {
                dead_enemies.push(index);
            } else if !enemy.entity.is_alive() {
                match &mut enemy.enemy_type {
                    EnemyType::Corpse { lifetime } => {
                        lifetime.change(-delta_time);
                        if !lifetime.is_alive() {
                            dead_enemies.push(index);
                        }
                        enemy.entity.color.a = lifetime.hp_frac() * 0.5;
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
                        enemy.enemy_type = EnemyType::Corpse {
                            lifetime: Health::new(CORPSE_LIFETIME),
                        }
                    }
                }
            }
        }
        dead_enemies.reverse();
        for dead_index in dead_enemies {
            self.enemies.remove(dead_index);
        }
    }
}
