//! # Gameplay Systems
//!
//! This module contains gameplay-specific systems:
//! - NPCs and AI
//! - Player interactions
//! - World events

pub mod npc;

pub use npc::{
    Npc, NpcType, NpcManager, AiState, NpcInstance, generate_npc_instances,
    NPC_MOVE_SPEED, NPC_WIDTH, NPC_HEIGHT, NPC_DETECTION_RANGE,
    NPC_WANDER_RADIUS, NPC_SPAWN_CHANCE,
};
