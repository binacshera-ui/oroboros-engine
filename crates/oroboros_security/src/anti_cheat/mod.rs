//! # Anti-Cheat Detection
//!
//! Server-side cheat detection using replay analysis.
//!
//! ## Detection Methods
//!
//! - **Aimbot**: Impossibly fast target acquisition, perfect tracking
//! - **Speedhack**: Movement faster than physics allows
//! - **Teleportation**: Position jumps without valid movement
//! - **Wallhack indicators**: Shooting through walls, pre-aiming

use oroboros_core::Position;
use oroboros_networking::protocol::PlayerInput;

/// Types of cheats we can detect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheatType {
    /// Aim assistance software.
    Aimbot,
    /// Speed modification.
    Speedhack,
    /// Position manipulation.
    Teleport,
    /// Possible wallhack (shooting through geometry).
    Wallhack,
    /// Generic suspicious behavior.
    Suspicious,
}

/// A cheat report.
#[derive(Clone, Debug)]
pub struct CheatReport {
    /// Player ID.
    pub player_id: u32,
    /// Type of cheat detected.
    pub cheat_type: CheatType,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f32,
    /// Tick when detected.
    pub tick: u32,
    /// Description of the detection.
    pub description: String,
    /// Evidence data.
    pub evidence: CheatEvidence,
}

/// Evidence for a cheat report.
#[derive(Clone, Debug, Default)]
pub struct CheatEvidence {
    /// Positions involved.
    pub positions: Vec<Position>,
    /// Speed values.
    pub speeds: Vec<f32>,
    /// Aim angles.
    pub aim_angles: Vec<(f32, f32)>,
    /// Additional notes.
    pub notes: Vec<String>,
}

/// Configuration for cheat detection.
#[derive(Clone, Debug)]
pub struct DetectorConfig {
    /// Maximum allowed movement speed.
    pub max_speed: f32,
    /// Maximum distance per tick.
    pub max_distance_per_tick: f32,
    /// Minimum time between aim snaps (ticks).
    pub min_aim_snap_ticks: u32,
    /// Maximum aim speed (degrees per tick).
    pub max_aim_speed: f32,
    /// Consecutive suspicious actions before flagging.
    pub suspicious_threshold: u32,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            max_speed: 15.0, // 10 base + 50% sprint margin
            max_distance_per_tick: 0.5, // ~30 units/sec at 60Hz
            min_aim_snap_ticks: 2,
            max_aim_speed: 180.0, // degrees per tick
            suspicious_threshold: 3,
        }
    }
}

/// Player state tracking for detection.
#[derive(Clone, Debug, Default)]
struct PlayerState {
    /// Last known position.
    last_position: Position,
    /// Last tick we saw this player.
    last_tick: u32,
    /// Last aim angles (yaw, pitch).
    last_aim: (f32, f32),
    /// Speed samples for analysis.
    speed_samples: Vec<f32>,
    /// Aim snap times (ticks between large aim changes).
    aim_snap_times: Vec<u32>,
    /// Consecutive suspicious actions.
    suspicious_count: u32,
}

/// Cheat detector.
pub struct CheatDetector {
    /// Configuration.
    config: DetectorConfig,
    /// Per-player state.
    player_states: Vec<PlayerState>,
    /// Generated reports.
    reports: Vec<CheatReport>,
    /// Current tick.
    current_tick: u32,
}

impl CheatDetector {
    /// Creates a new detector.
    #[must_use]
    pub fn new(config: DetectorConfig, max_players: usize) -> Self {
        Self {
            config,
            player_states: vec![PlayerState::default(); max_players],
            reports: Vec::new(),
            current_tick: 0,
        }
    }

    /// Sets the current tick.
    pub fn set_tick(&mut self, tick: u32) {
        self.current_tick = tick;
    }

    /// Analyzes a player's input and position.
    pub fn analyze(&mut self, player_id: u32, input: &PlayerInput, position: Position) {
        let idx = player_id as usize;
        if idx >= self.player_states.len() {
            return;
        }

        let state = &self.player_states[idx];
        let last_pos = state.last_position;
        let last_tick = state.last_tick;

        // Skip if this is the first frame
        if last_tick == 0 {
            self.player_states[idx].last_position = position;
            self.player_states[idx].last_tick = self.current_tick;
            self.player_states[idx].last_aim = (input.aim_yaw as f32, input.aim_pitch as f32);
            return;
        }

        let tick_delta = self.current_tick.saturating_sub(last_tick).max(1);

        // Check for speedhack
        self.check_speedhack(player_id, position, last_pos, tick_delta);

        // Check for teleportation
        self.check_teleport(player_id, position, last_pos, tick_delta);

        // Check for aimbot
        self.check_aimbot(player_id, input);

        // Update state
        let state = &mut self.player_states[idx];
        state.last_position = position;
        state.last_tick = self.current_tick;
        state.last_aim = (input.aim_yaw as f32, input.aim_pitch as f32);
    }

    /// Checks for speedhack.
    fn check_speedhack(&mut self, player_id: u32, pos: Position, last_pos: Position, tick_delta: u32) {
        let distance = calculate_distance(pos, last_pos);
        let speed = distance / tick_delta as f32;

        let idx = player_id as usize;
        self.player_states[idx].speed_samples.push(speed);

        // Keep last 60 samples
        if self.player_states[idx].speed_samples.len() > 60 {
            self.player_states[idx].speed_samples.remove(0);
        }

        // Check instantaneous speed
        if speed > self.config.max_speed {
            self.flag_suspicious(player_id);

            if self.player_states[idx].suspicious_count >= self.config.suspicious_threshold {
                self.reports.push(CheatReport {
                    player_id,
                    cheat_type: CheatType::Speedhack,
                    confidence: calculate_speedhack_confidence(speed, self.config.max_speed),
                    tick: self.current_tick,
                    description: format!(
                        "Speed {} exceeds maximum {} ({}x)",
                        speed,
                        self.config.max_speed,
                        speed / self.config.max_speed
                    ),
                    evidence: CheatEvidence {
                        positions: vec![last_pos, pos],
                        speeds: self.player_states[idx].speed_samples.clone(),
                        ..Default::default()
                    },
                });
            }
        } else {
            self.player_states[idx].suspicious_count = 
                self.player_states[idx].suspicious_count.saturating_sub(1);
        }
    }

    /// Checks for teleportation.
    fn check_teleport(&mut self, player_id: u32, pos: Position, last_pos: Position, tick_delta: u32) {
        let distance = calculate_distance(pos, last_pos);
        let max_possible = self.config.max_distance_per_tick * tick_delta as f32;

        if distance > max_possible * 3.0 {
            // Definite teleport
            self.reports.push(CheatReport {
                player_id,
                cheat_type: CheatType::Teleport,
                confidence: 1.0,
                tick: self.current_tick,
                description: format!(
                    "Teleported {} units in {} ticks (max possible: {})",
                    distance, tick_delta, max_possible
                ),
                evidence: CheatEvidence {
                    positions: vec![last_pos, pos],
                    ..Default::default()
                },
            });
        }
    }

    /// Checks for aimbot.
    fn check_aimbot(&mut self, player_id: u32, input: &PlayerInput) {
        let idx = player_id as usize;
        let state = &self.player_states[idx];

        let current_aim = (input.aim_yaw as f32, input.aim_pitch as f32);
        let last_aim = state.last_aim;

        // Calculate aim delta
        let yaw_delta = (current_aim.0 - last_aim.0).abs();
        let pitch_delta = (current_aim.1 - last_aim.1).abs();
        let aim_speed = (yaw_delta * yaw_delta + pitch_delta * pitch_delta).sqrt();

        // Check for inhuman aim snap
        if aim_speed > self.config.max_aim_speed {
            self.player_states[idx].aim_snap_times.push(self.current_tick);

            // Keep last 10 snaps
            if self.player_states[idx].aim_snap_times.len() > 10 {
                self.player_states[idx].aim_snap_times.remove(0);
            }

            // Check for pattern of rapid snaps
            let snap_times = &self.player_states[idx].aim_snap_times;
            if snap_times.len() >= 3 {
                let mut rapid_snaps = 0;
                for i in 1..snap_times.len() {
                    if snap_times[i] - snap_times[i-1] < self.config.min_aim_snap_ticks {
                        rapid_snaps += 1;
                    }
                }

                if rapid_snaps >= 2 {
                    self.reports.push(CheatReport {
                        player_id,
                        cheat_type: CheatType::Aimbot,
                        confidence: (rapid_snaps as f32 / snap_times.len() as f32).min(1.0),
                        tick: self.current_tick,
                        description: format!(
                            "Inhuman aim pattern: {} rapid snaps detected",
                            rapid_snaps
                        ),
                        evidence: CheatEvidence {
                            aim_angles: vec![last_aim, current_aim],
                            notes: vec![format!("Aim speed: {}", aim_speed)],
                            ..Default::default()
                        },
                    });
                }
            }
        }
    }

    /// Flags a player as suspicious.
    fn flag_suspicious(&mut self, player_id: u32) {
        let idx = player_id as usize;
        if idx < self.player_states.len() {
            self.player_states[idx].suspicious_count += 1;
        }
    }

    /// Returns all reports.
    #[must_use]
    pub fn reports(&self) -> &[CheatReport] {
        &self.reports
    }

    /// Clears all reports.
    pub fn clear_reports(&mut self) {
        self.reports.clear();
    }

    /// Takes ownership of reports.
    pub fn take_reports(&mut self) -> Vec<CheatReport> {
        std::mem::take(&mut self.reports)
    }

    /// Resets all player states.
    pub fn reset(&mut self) {
        for state in &mut self.player_states {
            *state = PlayerState::default();
        }
        self.reports.clear();
    }
}

impl Default for CheatDetector {
    fn default() -> Self {
        Self::new(DetectorConfig::default(), 500)
    }
}

/// Calculates distance between positions.
fn calculate_distance(a: Position, b: Position) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Calculates confidence for speedhack detection.
fn calculate_speedhack_confidence(speed: f32, max_speed: f32) -> f32 {
    let ratio = speed / max_speed;
    // Higher ratio = higher confidence
    ((ratio - 1.0) / 2.0).clamp(0.5, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speedhack_detection() {
        let mut detector = CheatDetector::new(DetectorConfig::default(), 10);

        // Normal movement
        detector.set_tick(1);
        detector.analyze(
            1,
            &PlayerInput::default(),
            Position::new(0.0, 0.0, 0.0),
        );

        detector.set_tick(2);
        detector.analyze(
            1,
            &PlayerInput::default(),
            Position::new(0.2, 0.0, 0.0),
        );

        // No reports for normal speed
        assert!(detector.reports().is_empty());

        // Teleport
        detector.set_tick(3);
        detector.analyze(
            1,
            &PlayerInput::default(),
            Position::new(100.0, 0.0, 0.0),
        );

        // Should have teleport report
        assert!(!detector.reports().is_empty());
        assert_eq!(detector.reports()[0].cheat_type, CheatType::Teleport);
    }

    #[test]
    fn test_aimbot_detection() {
        let config = DetectorConfig {
            max_aim_speed: 100.0,
            min_aim_snap_ticks: 2,
            ..Default::default()
        };
        let mut detector = CheatDetector::new(config, 10);

        // Initialize
        detector.set_tick(1);
        detector.analyze(
            1,
            &PlayerInput {
                aim_yaw: 0,
                aim_pitch: 0,
                ..Default::default()
            },
            Position::default(),
        );

        // Rapid aim snaps (inhuman)
        for i in 2..10 {
            detector.set_tick(i);
            detector.analyze(
                1,
                &PlayerInput {
                    aim_yaw: (i as i16 * 1000) % 3600, // Large jumps
                    aim_pitch: 0,
                    ..Default::default()
                },
                Position::default(),
            );
        }

        // Should detect aimbot pattern
        let aimbot_reports: Vec<_> = detector.reports()
            .iter()
            .filter(|r| r.cheat_type == CheatType::Aimbot)
            .collect();
        
        assert!(!aimbot_reports.is_empty());
    }
}
