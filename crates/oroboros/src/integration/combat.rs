//! # Combat Integration
//!
//! Server-side combat processing integrating Physics and Economy.

use oroboros_core::Position;

/// Attack command from client.
#[derive(Clone, Copy, Debug)]
pub struct AttackCommand {
    /// Client-side command sequence number.
    pub sequence: u32,
    /// Attacker entity ID.
    pub attacker_id: u32,
    /// Attack origin position.
    pub origin: Position,
    /// Attack direction (normalized).
    pub direction: (f32, f32, f32),
    /// Weapon ID.
    pub weapon_id: u8,
    /// Attack timestamp (client tick).
    pub client_tick: u32,
}

impl AttackCommand {
    /// Creates a new attack command.
    #[must_use]
    pub const fn new(
        sequence: u32,
        attacker_id: u32,
        origin: Position,
        direction: (f32, f32, f32),
    ) -> Self {
        Self {
            sequence,
            attacker_id,
            origin,
            direction,
            weapon_id: 0,
            client_tick: 0,
        }
    }

    /// Serializes to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.attacker_id.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.origin.x.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.origin.y.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.origin.z.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.direction.0.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.direction.1.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.direction.2.to_le_bytes());
        bytes
    }

    /// Deserializes from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 32 {
            return None;
        }
        Some(Self {
            sequence: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            attacker_id: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            origin: Position {
                x: f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
                y: f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
                z: f32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
                _padding: 0.0,
            },
            direction: (
                f32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
                f32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
                f32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
            ),
            weapon_id: 0,
            client_tick: 0,
        })
    }
}

/// Result of attack (sent back to client).
#[derive(Clone, Debug)]
pub struct AttackResult {
    /// Original command sequence.
    pub sequence: u32,
    /// Did the attack hit?
    pub hit: bool,
    /// Hit information (if hit).
    pub hit_info: Option<HitInfo>,
    /// Loot dropped (if any).
    pub loot: Option<LootDrop>,
    /// Server processing time in microseconds.
    pub processing_time_us: u64,
}

impl AttackResult {
    /// Creates a miss result.
    #[must_use]
    pub const fn miss(sequence: u32, processing_time_us: u64) -> Self {
        Self {
            sequence,
            hit: false,
            hit_info: None,
            loot: None,
            processing_time_us,
        }
    }

    /// Creates a hit result.
    #[must_use]
    pub const fn hit(sequence: u32, info: HitInfo, loot: Option<LootDrop>, processing_time_us: u64) -> Self {
        Self {
            sequence,
            hit: true,
            hit_info: Some(info),
            loot,
            processing_time_us,
        }
    }

    /// Serializes to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.push(u8::from(self.hit));
        bytes.extend_from_slice(&self.processing_time_us.to_le_bytes());

        if let Some(ref info) = self.hit_info {
            bytes.push(1); // Has hit info
            bytes.extend_from_slice(&info.target_id.to_le_bytes());
            bytes.extend_from_slice(&info.damage.to_le_bytes());
            bytes.push(info.damage_type as u8);
            bytes.push(u8::from(info.is_critical));
            bytes.push(u8::from(info.is_kill));
        } else {
            bytes.push(0); // No hit info
        }

        if let Some(ref loot) = self.loot {
            bytes.push(1); // Has loot
            bytes.extend_from_slice(&loot.item_id.to_le_bytes());
            bytes.extend_from_slice(&loot.quantity.to_le_bytes());
            bytes.push(loot.rarity as u8);
        } else {
            bytes.push(0); // No loot
        }

        bytes
    }

    /// Deserializes from bytes.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }

        let sequence = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let hit = bytes[4] != 0;
        let processing_time_us = u64::from_le_bytes([
            bytes[5], bytes[6], bytes[7], bytes[8],
            bytes[9], bytes[10], bytes[11], bytes[12],
        ]);

        let mut offset = 13;

        let hit_info = if bytes.get(offset) == Some(&1) {
            offset += 1;
            if offset + 11 > bytes.len() { return None; }
            let target_id = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]);
            offset += 4;
            let damage = u16::from_le_bytes([bytes[offset], bytes[offset+1]]);
            offset += 2;
            let damage_type = DamageType::from(bytes[offset]);
            offset += 1;
            let is_critical = bytes[offset] != 0;
            offset += 1;
            let is_kill = bytes[offset] != 0;
            offset += 1;
            Some(HitInfo { target_id, damage, damage_type, is_critical, is_kill })
        } else {
            offset += 1;
            None
        };

        let loot = if offset < bytes.len() && bytes.get(offset) == Some(&1) {
            offset += 1;
            if offset + 7 > bytes.len() { return None; }
            let item_id = u32::from_le_bytes([bytes[offset], bytes[offset+1], bytes[offset+2], bytes[offset+3]]);
            offset += 4;
            let quantity = u16::from_le_bytes([bytes[offset], bytes[offset+1]]);
            offset += 2;
            let rarity = LootRarity::from(bytes[offset]);
            Some(LootDrop { item_id, quantity, rarity })
        } else {
            None
        };

        Some(Self {
            sequence,
            hit,
            hit_info,
            loot,
            processing_time_us,
        })
    }
}

/// Hit information.
#[derive(Clone, Copy, Debug)]
pub struct HitInfo {
    /// Target entity that was hit.
    pub target_id: u32,
    /// Damage dealt.
    pub damage: u16,
    /// Type of damage.
    pub damage_type: DamageType,
    /// Was it a critical hit?
    pub is_critical: bool,
    /// Did the target die?
    pub is_kill: bool,
}

/// Damage types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DamageType {
    /// Physical damage.
    Physical = 0,
    /// Fire damage.
    Fire = 1,
    /// Ice damage.
    Ice = 2,
    /// Lightning damage.
    Lightning = 3,
    /// Dragon damage.
    Dragon = 4,
}

impl From<u8> for DamageType {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Physical,
            1 => Self::Fire,
            2 => Self::Ice,
            3 => Self::Lightning,
            4 => Self::Dragon,
            _ => Self::Physical,
        }
    }
}

/// Loot drop.
#[derive(Clone, Copy, Debug)]
pub struct LootDrop {
    /// Item ID.
    pub item_id: u32,
    /// Quantity.
    pub quantity: u16,
    /// Rarity.
    pub rarity: LootRarity,
}

/// Loot rarity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LootRarity {
    /// Common loot.
    Common = 0,
    /// Uncommon loot.
    Uncommon = 1,
    /// Rare loot.
    Rare = 2,
    /// Epic loot.
    Epic = 3,
    /// Legendary loot.
    Legendary = 4,
}

impl From<u8> for LootRarity {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Common,
            1 => Self::Uncommon,
            2 => Self::Rare,
            3 => Self::Epic,
            4 => Self::Legendary,
            _ => Self::Common,
        }
    }
}

/// Server-side entity for hit detection.
#[derive(Clone, Copy, Debug)]
pub struct ServerEntity {
    /// Entity ID.
    pub id: u32,
    /// Position.
    pub position: Position,
    /// Hitbox radius.
    pub hitbox_radius: f32,
    /// Current health.
    pub health: u16,
    /// Maximum health.
    pub max_health: u16,
    /// Is alive?
    pub alive: bool,
}

impl ServerEntity {
    /// Creates a new server entity.
    #[must_use]
    pub const fn new(id: u32, position: Position, health: u16) -> Self {
        Self {
            id,
            position,
            hitbox_radius: 0.5,
            health,
            max_health: health,
            alive: true,
        }
    }
}

/// Combat processor - integrates Physics + Economy.
pub struct CombatProcessor {
    /// All entities in the world.
    entities: Vec<ServerEntity>,
    /// Base weapon damage.
    base_damage: u16,
    /// Critical hit chance (0-100).
    crit_chance: u8,
    /// Critical hit multiplier (x100, so 200 = 2.0x).
    crit_multiplier: u16,
    /// Loot drop chance (0-100).
    loot_chance: u8,
    /// RNG state.
    rng_state: u64,
}

impl CombatProcessor {
    /// Creates a new combat processor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entities: Vec::with_capacity(1000),
            base_damage: 25,
            crit_chance: 10,
            crit_multiplier: 200,
            loot_chance: 30,
            rng_state: 12345,
        }
    }

    /// Adds an entity.
    pub fn add_entity(&mut self, entity: ServerEntity) {
        self.entities.push(entity);
    }

    /// Gets mutable entity by ID.
    fn get_entity_mut(&mut self, id: u32) -> Option<&mut ServerEntity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    /// Simple RNG.
    fn rand(&mut self) -> u32 {
        self.rng_state = self.rng_state.wrapping_mul(48271).wrapping_rem(2_147_483_647);
        self.rng_state as u32
    }

    /// Processes an attack command.
    ///
    /// Returns result in microseconds.
    pub fn process_attack(&mut self, command: &AttackCommand) -> AttackResult {
        let start = std::time::Instant::now();

        // Phase 1: Physics - Raycast to find hit
        let hit_result = self.raycast_hit(
            command.origin,
            command.direction,
            command.attacker_id,
        );

        match hit_result {
            Some((target_id, _distance)) => {
                // Phase 2: Economy - Calculate damage
                let is_critical = (self.rand() % 100) < u32::from(self.crit_chance);
                let mut damage = self.base_damage;

                if is_critical {
                    damage = (u32::from(damage) * u32::from(self.crit_multiplier) / 100) as u16;
                }

                // Apply damage to target
                let is_kill = if let Some(target) = self.get_entity_mut(target_id) {
                    if target.health <= damage {
                        target.health = 0;
                        target.alive = false;
                        true
                    } else {
                        target.health -= damage;
                        false
                    }
                } else {
                    false
                };

                // Phase 3: Loot calculation (if kill)
                let loot = if is_kill && (self.rand() % 100) < u32::from(self.loot_chance) {
                    let rarity_roll = self.rand() % 100;
                    let rarity = if rarity_roll < 60 {
                        LootRarity::Common
                    } else if rarity_roll < 85 {
                        LootRarity::Uncommon
                    } else if rarity_roll < 95 {
                        LootRarity::Rare
                    } else if rarity_roll < 99 {
                        LootRarity::Epic
                    } else {
                        LootRarity::Legendary
                    };

                    Some(LootDrop {
                        item_id: self.rand(),
                        quantity: 1,
                        rarity,
                    })
                } else {
                    None
                };

                let hit_info = HitInfo {
                    target_id,
                    damage,
                    damage_type: DamageType::Physical,
                    is_critical,
                    is_kill,
                };

                let processing_time = start.elapsed().as_micros() as u64;
                AttackResult::hit(command.sequence, hit_info, loot, processing_time)
            }
            None => {
                let processing_time = start.elapsed().as_micros() as u64;
                AttackResult::miss(command.sequence, processing_time)
            }
        }
    }

    /// Performs a raycast to find the first hit entity.
    fn raycast_hit(
        &self,
        origin: Position,
        direction: (f32, f32, f32),
        attacker_id: u32,
    ) -> Option<(u32, f32)> {
        let mut closest_hit: Option<(u32, f32)> = None;

        for entity in &self.entities {
            // Skip self and dead entities
            if entity.id == attacker_id || !entity.alive {
                continue;
            }

            // Simple sphere intersection test
            if let Some(distance) = self.ray_sphere_intersect(
                origin,
                direction,
                entity.position,
                entity.hitbox_radius,
            ) {
                if closest_hit.is_none() || distance < closest_hit.unwrap().1 {
                    closest_hit = Some((entity.id, distance));
                }
            }
        }

        closest_hit
    }

    /// Ray-sphere intersection.
    fn ray_sphere_intersect(
        &self,
        origin: Position,
        direction: (f32, f32, f32),
        sphere_center: Position,
        radius: f32,
    ) -> Option<f32> {
        let ox = origin.x - sphere_center.x;
        let oy = origin.y - sphere_center.y;
        let oz = origin.z - sphere_center.z;

        let a = direction.0 * direction.0 + direction.1 * direction.1 + direction.2 * direction.2;
        let b = 2.0 * (ox * direction.0 + oy * direction.1 + oz * direction.2);
        let c = ox * ox + oy * oy + oz * oz - radius * radius;

        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None;
        }

        let t = (-b - discriminant.sqrt()) / (2.0 * a);

        if t > 0.0 {
            Some(t)
        } else {
            None
        }
    }
}

impl Default for CombatProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attack_hit() {
        let mut processor = CombatProcessor::new();
        
        // Add attacker
        processor.add_entity(ServerEntity::new(1, Position::new(0.0, 0.0, 0.0), 100));
        
        // Add target in front
        processor.add_entity(ServerEntity::new(2, Position::new(5.0, 0.0, 0.0), 100));

        let command = AttackCommand::new(
            1,
            1,
            Position::new(0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0), // Shooting towards target
        );

        let result = processor.process_attack(&command);

        assert!(result.hit, "Should hit target");
        assert!(result.hit_info.is_some());
        assert_eq!(result.hit_info.unwrap().target_id, 2);
        println!("Processing time: {} μs", result.processing_time_us);
    }

    #[test]
    fn test_processing_time() {
        let mut processor = CombatProcessor::new();
        
        // Add many entities
        for i in 0..100 {
            processor.add_entity(ServerEntity::new(
                i,
                Position::new(i as f32, 0.0, 0.0),
                100,
            ));
        }

        let command = AttackCommand::new(
            1,
            0,
            Position::new(0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
        );

        let result = processor.process_attack(&command);

        // Processing should be under 100μs
        assert!(result.processing_time_us < 100, 
            "Processing took {} μs, should be < 100μs", result.processing_time_us);
        
        println!("Processing time with 100 entities: {} μs", result.processing_time_us);
    }
}
