//! # Dragon Module
//!
//! The Algorithmic Dragon - a living representation of the liquidity pool.
//!
//! ## Overview
//!
//! The Dragon is not a typical boss. It's an algorithm that:
//! - Responds to real-time market data
//! - Creates deterministic gameplay events
//! - Serves as the game's primary "sink" mechanism
//!
//! ## Submodules
//!
//! - `state_machine`: Core state machine logic (tick-based, for compatibility)
//! - `event_driven`: Event-driven dragon (ZERO arbitrage window)
//!
//! ## IMPORTANT
//!
//! For production, use `event_driven` module. The tick-based state machine
//! creates a 16ms arbitrage window that MEV bots can exploit.

pub mod state_machine;
pub mod event_driven;

pub use state_machine::{
    DragonStateMachine, 
    DragonConfig, 
    MarketData, 
    MockMarketDataSource, 
    VolatilityPattern,
};

pub use event_driven::{
    EventDrivenDragon,
    EventDragonConfig,
    SharedDragonState,
    MarketEvent,
    MarketEventType,
    DragonBroadcast,
    DragonStateValue,
    create_event_dragon_system,
};

// Re-export dragon state from networking
pub use oroboros_networking::protocol::DragonState;
