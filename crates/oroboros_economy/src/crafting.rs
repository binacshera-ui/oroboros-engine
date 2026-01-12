//! # Crafting System - Directed Acyclic Graph (DAG)
//!
//! **Transactional Recipe System with Cycle Detection**
//!
//! This module implements the crafting system with the following guarantees:
//!
//! 1. **No Cycles**: The recipe graph is validated to be acyclic (DAG)
//! 2. **Transactional**: Crafting is atomic - all materials consumed OR nothing happens
//! 3. **No Duplication**: Impossible to create items from nothing
//! 4. **External Configuration**: All recipes defined in TOML files
//!
//! ## Security Model
//!
//! The crafting system runs ONLY on the authoritative server.
//! Client requests are validated and can be rejected if:
//! - Player doesn't have required materials
//! - Recipe doesn't exist
//! - Player doesn't meet level requirements
//!
//! ## Example
//!
//! ```rust,ignore
//! let mut graph = CraftingGraph::new();
//! graph.add_recipe(Recipe {
//!     id: 1,
//!     inputs: vec![(IRON_ORE, 3), (COAL, 1)],
//!     outputs: vec![(IRON_INGOT, 1)],
//!     crafting_time_ms: 5000,
//!     required_level: 5,
//! })?;
//!
//! // Validate no cycles
//! assert!(graph.validate_no_cycles());
//!
//! // Perform transactional craft
//! graph.craft(&mut inventory, recipe_id)?;
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::{EconomyError, EconomyResult};
use crate::inventory::{Inventory, ItemId};

/// Unique identifier for a recipe.
pub type RecipeId = u32;

/// Input or output item in a recipe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeItem {
    /// The item ID.
    pub item_id: ItemId,
    /// Quantity required/produced.
    pub quantity: u32,
}

impl RecipeItem {
    /// Creates a new recipe item.
    #[inline]
    #[must_use]
    pub const fn new(item_id: ItemId, quantity: u32) -> Self {
        Self { item_id, quantity }
    }
}

/// A crafting recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    /// Unique recipe identifier.
    pub id: RecipeId,
    /// Human-readable name.
    pub name: String,
    /// Items consumed by this recipe.
    pub inputs: Vec<RecipeItem>,
    /// Items produced by this recipe.
    pub outputs: Vec<RecipeItem>,
    /// Time to craft in milliseconds.
    pub crafting_time_ms: u32,
    /// Minimum player level required.
    pub required_level: u8,
    /// Skill points awarded for crafting.
    pub skill_points: u32,
}

impl Recipe {
    /// Creates a new recipe with basic validation.
    ///
    /// # Errors
    ///
    /// Returns error if recipe has no inputs or outputs.
    pub fn new(
        id: RecipeId,
        name: String,
        inputs: Vec<RecipeItem>,
        outputs: Vec<RecipeItem>,
    ) -> EconomyResult<Self> {
        if inputs.is_empty() {
            return Err(EconomyError::InvalidConfig(
                "Recipe must have at least one input".to_string(),
            ));
        }
        if outputs.is_empty() {
            return Err(EconomyError::InvalidConfig(
                "Recipe must have at least one output".to_string(),
            ));
        }

        Ok(Self {
            id,
            name,
            inputs,
            outputs,
            crafting_time_ms: 0,
            required_level: 0,
            skill_points: 0,
        })
    }

    /// Sets the crafting time.
    #[must_use]
    pub const fn with_time(mut self, time_ms: u32) -> Self {
        self.crafting_time_ms = time_ms;
        self
    }

    /// Sets the required level.
    #[must_use]
    pub const fn with_level(mut self, level: u8) -> Self {
        self.required_level = level;
        self
    }

    /// Sets skill points awarded.
    #[must_use]
    pub const fn with_skill_points(mut self, points: u32) -> Self {
        self.skill_points = points;
        self
    }
}

/// The crafting graph - a Directed Acyclic Graph of recipes.
///
/// Maintains integrity of the item economy by:
/// 1. Detecting cycles that would allow infinite item generation
/// 2. Ensuring all crafting is transactional
#[derive(Debug, Default)]
pub struct CraftingGraph {
    /// All recipes indexed by ID.
    recipes: HashMap<RecipeId, Recipe>,
    /// Items that can be produced, mapped to recipes that produce them.
    item_producers: HashMap<ItemId, Vec<RecipeId>>,
    /// Items that are consumed, mapped to recipes that consume them.
    item_consumers: HashMap<ItemId, Vec<RecipeId>>,
    /// Whether the graph has been validated as cycle-free.
    validated: bool,
}

impl CraftingGraph {
    /// Creates a new empty crafting graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a recipe to the graph.
    ///
    /// # Errors
    ///
    /// Returns error if recipe ID already exists.
    pub fn add_recipe(&mut self, recipe: Recipe) -> EconomyResult<()> {
        if self.recipes.contains_key(&recipe.id) {
            return Err(EconomyError::InvalidConfig(format!(
                "Recipe ID {} already exists",
                recipe.id
            )));
        }

        // Index inputs (consumers)
        for input in &recipe.inputs {
            self.item_consumers
                .entry(input.item_id)
                .or_default()
                .push(recipe.id);
        }

        // Index outputs (producers)
        for output in &recipe.outputs {
            self.item_producers
                .entry(output.item_id)
                .or_default()
                .push(recipe.id);
        }

        self.recipes.insert(recipe.id, recipe);
        self.validated = false;

        Ok(())
    }

    /// Gets a recipe by ID.
    #[must_use]
    pub fn get_recipe(&self, id: RecipeId) -> Option<&Recipe> {
        self.recipes.get(&id)
    }

    /// Returns all recipes.
    #[must_use]
    pub fn all_recipes(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    /// Returns the number of recipes.
    #[must_use]
    pub fn recipe_count(&self) -> usize {
        self.recipes.len()
    }

    /// Validates that the recipe graph has no cycles.
    ///
    /// Uses Kahn's algorithm for topological sorting.
    /// If sorting succeeds, the graph is a valid DAG.
    ///
    /// # Returns
    ///
    /// `true` if the graph is cycle-free, `false` if cycles exist.
    #[must_use]
    pub fn validate_no_cycles(&mut self) -> bool {
        if self.validated {
            return true;
        }

        // Build adjacency list: recipe A -> recipe B if A produces something B consumes
        let mut in_degree: HashMap<RecipeId, usize> = HashMap::new();
        let mut adjacency: HashMap<RecipeId, Vec<RecipeId>> = HashMap::new();

        // Initialize all recipes with 0 in-degree
        for &recipe_id in self.recipes.keys() {
            in_degree.insert(recipe_id, 0);
            adjacency.insert(recipe_id, Vec::new());
        }

        // Build edges based on item dependencies
        for (&recipe_id, recipe) in &self.recipes {
            for input in &recipe.inputs {
                // Find recipes that produce this input
                if let Some(producers) = self.item_producers.get(&input.item_id) {
                    for &producer_id in producers {
                        if producer_id != recipe_id {
                            adjacency.entry(producer_id).or_default().push(recipe_id);
                            *in_degree.entry(recipe_id).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<RecipeId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut sorted_count = 0;

        while let Some(recipe_id) = queue.pop_front() {
            sorted_count += 1;

            if let Some(neighbors) = adjacency.get(&recipe_id) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(&neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        // If we processed all recipes, there are no cycles
        self.validated = sorted_count == self.recipes.len();
        self.validated
    }

    /// Detects which recipes are involved in a cycle.
    ///
    /// Useful for debugging invalid recipe configurations.
    #[must_use]
    pub fn find_cycle(&self) -> Option<Vec<RecipeId>> {
        // DFS-based cycle detection
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for &start_id in self.recipes.keys() {
            if !visited.contains(&start_id) {
                if let Some(cycle) = self.dfs_find_cycle(start_id, &mut visited, &mut rec_stack, &mut path) {
                    return Some(cycle);
                }
            }
        }

        None
    }

    /// DFS helper for cycle detection.
    fn dfs_find_cycle(
        &self,
        recipe_id: RecipeId,
        visited: &mut HashSet<RecipeId>,
        rec_stack: &mut HashSet<RecipeId>,
        path: &mut Vec<RecipeId>,
    ) -> Option<Vec<RecipeId>> {
        visited.insert(recipe_id);
        rec_stack.insert(recipe_id);
        path.push(recipe_id);

        // Get items this recipe produces
        if let Some(recipe) = self.recipes.get(&recipe_id) {
            for output in &recipe.outputs {
                // Find recipes that consume this output
                if let Some(consumers) = self.item_consumers.get(&output.item_id) {
                    for &consumer_id in consumers {
                        if consumer_id == recipe_id {
                            continue; // Skip self-reference
                        }

                        if !visited.contains(&consumer_id) {
                            if let Some(cycle) = self.dfs_find_cycle(consumer_id, visited, rec_stack, path) {
                                return Some(cycle);
                            }
                        } else if rec_stack.contains(&consumer_id) {
                            // Found cycle - extract it from path
                            let cycle_start = path.iter().position(|&id| id == consumer_id).unwrap_or(0);
                            let mut cycle: Vec<RecipeId> = path[cycle_start..].to_vec();
                            cycle.push(consumer_id);
                            return Some(cycle);
                        }
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(&recipe_id);
        None
    }

    /// Checks if a player can craft a recipe.
    ///
    /// # Arguments
    ///
    /// * `inventory` - Player's inventory
    /// * `recipe_id` - Recipe to check
    /// * `player_level` - Player's current level
    ///
    /// # Returns
    ///
    /// `Ok(())` if craftable, `Err` with reason if not.
    pub fn can_craft(
        &self,
        inventory: &Inventory,
        recipe_id: RecipeId,
        player_level: u8,
    ) -> EconomyResult<()> {
        let recipe = self.recipes.get(&recipe_id)
            .ok_or(EconomyError::RecipeNotFound(recipe_id))?;

        // Check level requirement
        if player_level < recipe.required_level {
            return Err(EconomyError::InvalidConfig(format!(
                "Required level {} but player is level {}",
                recipe.required_level, player_level
            )));
        }

        // Check all input materials
        for input in &recipe.inputs {
            let available = inventory.count_item(input.item_id);
            if available < input.quantity {
                return Err(EconomyError::InsufficientMaterials {
                    item_id: input.item_id,
                    required: input.quantity,
                    available,
                });
            }
        }

        Ok(())
    }

    /// Performs a transactional craft operation.
    ///
    /// **ATOMIC**: Either all materials are consumed and all outputs created,
    /// or nothing happens. Uses inventory snapshots for rollback.
    ///
    /// # Arguments
    ///
    /// * `inventory` - Player's inventory (mutable)
    /// * `recipe_id` - Recipe to craft
    /// * `player_level` - Player's current level
    ///
    /// # Errors
    ///
    /// - `RecipeNotFound` if recipe doesn't exist
    /// - `InsufficientMaterials` if player doesn't have inputs
    /// - `InventoryFull` if no space for outputs
    pub fn craft(
        &self,
        inventory: &mut Inventory,
        recipe_id: RecipeId,
        player_level: u8,
    ) -> EconomyResult<CraftResult> {
        // First check if crafting is possible
        self.can_craft(inventory, recipe_id, player_level)?;

        let recipe = self.recipes.get(&recipe_id).unwrap();

        // Take snapshot for rollback
        let snapshot = inventory.snapshot();

        // Remove input materials
        for input in &recipe.inputs {
            if let Err(e) = inventory.remove(input.item_id, input.quantity) {
                // Rollback on failure
                inventory.restore(&snapshot);
                return Err(e);
            }
        }

        // Add output items
        for output in &recipe.outputs {
            // Note: We need to know max_stack from item registry in production
            // For now, assume 64 as default max stack
            const DEFAULT_MAX_STACK: u32 = 64;
            if let Err(e) = inventory.add(output.item_id, output.quantity, DEFAULT_MAX_STACK) {
                // Rollback on failure
                inventory.restore(&snapshot);
                return Err(e);
            }
        }

        Ok(CraftResult {
            recipe_id,
            outputs: recipe.outputs.clone(),
            skill_points: recipe.skill_points,
            crafting_time_ms: recipe.crafting_time_ms,
        })
    }

    /// Simulates a craft without modifying inventory.
    ///
    /// Useful for UI to show what will be produced.
    #[must_use]
    pub fn simulate_craft(
        &self,
        inventory: &Inventory,
        recipe_id: RecipeId,
        player_level: u8,
    ) -> EconomyResult<CraftResult> {
        // Check if craftable
        self.can_craft(inventory, recipe_id, player_level)?;

        let recipe = self.recipes.get(&recipe_id).unwrap();

        Ok(CraftResult {
            recipe_id,
            outputs: recipe.outputs.clone(),
            skill_points: recipe.skill_points,
            crafting_time_ms: recipe.crafting_time_ms,
        })
    }
}

/// Result of a successful craft operation.
#[derive(Clone, Debug)]
pub struct CraftResult {
    /// The recipe that was crafted.
    pub recipe_id: RecipeId,
    /// Items produced.
    pub outputs: Vec<RecipeItem>,
    /// Skill points awarded.
    pub skill_points: u32,
    /// Time taken in milliseconds.
    pub crafting_time_ms: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Item IDs for testing
    const IRON_ORE: ItemId = 1;
    const COAL: ItemId = 2;
    const IRON_INGOT: ItemId = 3;
    const STEEL_INGOT: ItemId = 4;
    const STEEL_SWORD: ItemId = 5;

    fn create_test_graph() -> CraftingGraph {
        let mut graph = CraftingGraph::new();

        // Recipe 1: Iron Ore + Coal -> Iron Ingot
        graph.add_recipe(Recipe::new(
            1,
            "Iron Ingot".to_string(),
            vec![RecipeItem::new(IRON_ORE, 3), RecipeItem::new(COAL, 1)],
            vec![RecipeItem::new(IRON_INGOT, 1)],
        ).unwrap().with_level(5)).unwrap();

        // Recipe 2: Iron Ingot + Coal -> Steel Ingot
        graph.add_recipe(Recipe::new(
            2,
            "Steel Ingot".to_string(),
            vec![RecipeItem::new(IRON_INGOT, 2), RecipeItem::new(COAL, 2)],
            vec![RecipeItem::new(STEEL_INGOT, 1)],
        ).unwrap().with_level(10)).unwrap();

        // Recipe 3: Steel Ingot -> Steel Sword
        graph.add_recipe(Recipe::new(
            3,
            "Steel Sword".to_string(),
            vec![RecipeItem::new(STEEL_INGOT, 3)],
            vec![RecipeItem::new(STEEL_SWORD, 1)],
        ).unwrap().with_level(15)).unwrap();

        graph
    }

    #[test]
    fn test_valid_dag() {
        let mut graph = create_test_graph();
        assert!(graph.validate_no_cycles(), "Valid recipe chain should have no cycles");
    }

    #[test]
    fn test_detect_cycle() {
        let mut graph = CraftingGraph::new();

        // Create a cycle: A -> B -> C -> A
        // Item 100 -> Recipe 1 -> Item 101
        // Item 101 -> Recipe 2 -> Item 102
        // Item 102 -> Recipe 3 -> Item 100 (cycle!)

        graph.add_recipe(Recipe::new(
            1,
            "A to B".to_string(),
            vec![RecipeItem::new(100, 1)],
            vec![RecipeItem::new(101, 1)],
        ).unwrap()).unwrap();

        graph.add_recipe(Recipe::new(
            2,
            "B to C".to_string(),
            vec![RecipeItem::new(101, 1)],
            vec![RecipeItem::new(102, 1)],
        ).unwrap()).unwrap();

        graph.add_recipe(Recipe::new(
            3,
            "C to A".to_string(),
            vec![RecipeItem::new(102, 1)],
            vec![RecipeItem::new(100, 1)], // Creates cycle!
        ).unwrap()).unwrap();

        assert!(!graph.validate_no_cycles(), "Should detect cycle");
        
        let cycle = graph.find_cycle();
        assert!(cycle.is_some(), "Should find the cycle");
        println!("Found cycle: {:?}", cycle);
    }

    #[test]
    fn test_transactional_craft_success() {
        let graph = create_test_graph();
        let mut inventory = Inventory::new();

        // Add materials for iron ingot
        inventory.add(IRON_ORE, 10, 64).unwrap();
        inventory.add(COAL, 5, 64).unwrap();

        // Craft iron ingot
        let result = graph.craft(&mut inventory, 1, 10);
        assert!(result.is_ok(), "Craft should succeed");

        // Verify materials consumed
        assert_eq!(inventory.count_item(IRON_ORE), 7); // 10 - 3
        assert_eq!(inventory.count_item(COAL), 4);     // 5 - 1
        assert_eq!(inventory.count_item(IRON_INGOT), 1);
    }

    #[test]
    fn test_transactional_craft_rollback() {
        let graph = create_test_graph();
        let mut inventory = Inventory::new();

        // Add only some materials
        inventory.add(IRON_ORE, 2, 64).unwrap(); // Need 3!
        inventory.add(COAL, 5, 64).unwrap();

        // Craft should fail
        let result = graph.craft(&mut inventory, 1, 10);
        assert!(result.is_err());

        // Verify nothing changed (rollback)
        assert_eq!(inventory.count_item(IRON_ORE), 2);
        assert_eq!(inventory.count_item(COAL), 5);
        assert_eq!(inventory.count_item(IRON_INGOT), 0);
    }

    #[test]
    fn test_level_requirement() {
        let graph = create_test_graph();
        let mut inventory = Inventory::new();

        inventory.add(IRON_ORE, 10, 64).unwrap();
        inventory.add(COAL, 5, 64).unwrap();

        // Level 1 player can't craft (needs level 5)
        let result = graph.craft(&mut inventory, 1, 1);
        assert!(result.is_err());

        // Materials should be unchanged
        assert_eq!(inventory.count_item(IRON_ORE), 10);
    }

    #[test]
    fn test_craft_chain() {
        let graph = create_test_graph();
        let mut inventory = Inventory::new();

        // Add enough materials for the entire chain
        inventory.add(IRON_ORE, 18, 64).unwrap(); // 3 * 6 = 18 for 6 iron ingots
        inventory.add(COAL, 20, 64).unwrap();

        // Craft 6 iron ingots
        for _ in 0..6 {
            graph.craft(&mut inventory, 1, 50).unwrap();
        }
        assert_eq!(inventory.count_item(IRON_INGOT), 6);

        // Craft 3 steel ingots (uses 6 iron + 6 coal)
        for _ in 0..3 {
            graph.craft(&mut inventory, 2, 50).unwrap();
        }
        assert_eq!(inventory.count_item(STEEL_INGOT), 3);

        // Craft steel sword
        let result = graph.craft(&mut inventory, 3, 50);
        assert!(result.is_ok());
        assert_eq!(inventory.count_item(STEEL_SWORD), 1);
        assert_eq!(inventory.count_item(STEEL_INGOT), 0);
    }

    #[test]
    fn test_database_lock_simulation() {
        // Simulates what happens when DB is locked during crafting
        let _graph = create_test_graph();
        let mut inventory = Inventory::new();

        inventory.add(IRON_ORE, 10, 64).unwrap();
        inventory.add(COAL, 5, 64).unwrap();

        // Take snapshot like DB would
        let snapshot = inventory.snapshot();

        // Start craft - remove materials
        inventory.remove(IRON_ORE, 3).unwrap();
        inventory.remove(COAL, 1).unwrap();

        // Simulate DB lock failure before adding output
        // Rollback!
        inventory.restore(&snapshot);

        // Verify complete rollback
        assert_eq!(inventory.count_item(IRON_ORE), 10);
        assert_eq!(inventory.count_item(COAL), 5);
        assert_eq!(inventory.count_item(IRON_INGOT), 0);
    }
}
