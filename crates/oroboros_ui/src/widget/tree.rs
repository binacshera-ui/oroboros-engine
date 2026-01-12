//! Widget tree for hierarchical UI management.

use super::{WidgetId, WidgetState};
use std::collections::HashMap;

/// Manages the widget hierarchy.
pub struct WidgetTree {
    /// Widget states indexed by ID.
    widgets: HashMap<WidgetId, WidgetState>,
    /// Root widget IDs.
    roots: Vec<WidgetId>,
    /// Parent-child relationships.
    children: HashMap<WidgetId, Vec<WidgetId>>,
    /// ID counter for generating unique IDs.
    next_id: u64,
}

impl WidgetTree {
    /// Creates a new empty widget tree.
    #[must_use]
    pub fn new() -> Self {
        Self {
            widgets: HashMap::with_capacity(256),
            roots: Vec::with_capacity(16),
            children: HashMap::with_capacity(256),
            next_id: 1,
        }
    }
    
    /// Generates a new unique widget ID.
    pub fn next_id(&mut self) -> WidgetId {
        let id = WidgetId::new(self.next_id);
        self.next_id += 1;
        id
    }
    
    /// Registers a root widget.
    pub fn add_root(&mut self, state: WidgetState) {
        let id = state.id;
        self.widgets.insert(id, state);
        self.roots.push(id);
        self.children.insert(id, Vec::new());
    }
    
    /// Adds a child widget to a parent.
    pub fn add_child(&mut self, parent: WidgetId, state: WidgetState) {
        let id = state.id;
        let mut state = state;
        state.parent = Some(parent);
        
        self.widgets.insert(id, state);
        self.children.entry(parent).or_default().push(id);
        self.children.insert(id, Vec::new());
    }
    
    /// Removes a widget and all its children.
    pub fn remove(&mut self, id: WidgetId) {
        // Remove all children first
        if let Some(children) = self.children.remove(&id) {
            for child in children {
                self.remove(child);
            }
        }
        
        // Remove from parent's children list
        if let Some(state) = self.widgets.get(&id) {
            if let Some(parent) = state.parent {
                if let Some(siblings) = self.children.get_mut(&parent) {
                    siblings.retain(|&c| c != id);
                }
            }
        }
        
        // Remove from roots if it was a root
        self.roots.retain(|&r| r != id);
        
        // Remove the widget itself
        self.widgets.remove(&id);
    }
    
    /// Gets a widget state by ID.
    #[must_use]
    pub fn get(&self, id: WidgetId) -> Option<&WidgetState> {
        self.widgets.get(&id)
    }
    
    /// Gets mutable access to a widget state.
    #[must_use]
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut WidgetState> {
        self.widgets.get_mut(&id)
    }
    
    /// Returns the children of a widget.
    #[must_use]
    pub fn children(&self, id: WidgetId) -> &[WidgetId] {
        self.children.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }
    
    /// Returns all root widgets.
    #[must_use]
    pub fn roots(&self) -> &[WidgetId] {
        &self.roots
    }
    
    /// Returns all widget IDs in depth-first order.
    pub fn iter_dfs(&self) -> impl Iterator<Item = WidgetId> + '_ {
        WidgetDfsIterator {
            tree: self,
            stack: self.roots.iter().rev().copied().collect(),
        }
    }
    
    /// Returns all widget IDs in reverse depth-first order (for hit testing).
    pub fn iter_reverse(&self) -> impl Iterator<Item = WidgetId> + '_ {
        // Collect all in DFS order then reverse
        let all: Vec<_> = self.iter_dfs().collect();
        all.into_iter().rev()
    }
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Depth-first iterator over widget tree.
struct WidgetDfsIterator<'a> {
    tree: &'a WidgetTree,
    stack: Vec<WidgetId>,
}

impl Iterator for WidgetDfsIterator<'_> {
    type Item = WidgetId;
    
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        
        // Push children in reverse order so they're processed left-to-right
        if let Some(children) = self.tree.children.get(&id) {
            for &child in children.iter().rev() {
                self.stack.push(child);
            }
        }
        
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tree_hierarchy() {
        let mut tree = WidgetTree::new();
        
        let root_id = tree.next_id();
        tree.add_root(WidgetState::new(root_id));
        
        let child1_id = tree.next_id();
        tree.add_child(root_id, WidgetState::new(child1_id));
        
        let child2_id = tree.next_id();
        tree.add_child(root_id, WidgetState::new(child2_id));
        
        assert_eq!(tree.children(root_id).len(), 2);
        assert_eq!(tree.roots().len(), 1);
    }
}
