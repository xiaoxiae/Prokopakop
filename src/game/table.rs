use crate::game::board::{BoardMove, Game};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Exact,      // PV node - exact evaluation
    LowerBound, // Cut node - beta cutoff occurred (evaluation >= actual)
    UpperBound, // All node - no move improved alpha (evaluation <= actual)
}

#[derive(Debug, Clone)]
pub struct TTEntry {
    pub key: u64,             // Zobrist key for verification
    pub depth: u8,            // Search depth
    pub evaluation: f32,      // Stored evaluation
    pub best_move: BoardMove, // Best move found
    pub node_type: NodeType,  // Type of node
    pub age: u8,              // Search generation for replacement strategy
}

impl Default for TTEntry {
    fn default() -> Self {
        Self {
            key: 0,
            depth: 0,
            evaluation: 0.0,
            best_move: BoardMove::default(),
            node_type: NodeType::Exact,
            age: 0,
        }
    }
}

pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    size_mask: usize,
    generation: u8,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>();
        let total_entries = (size_mb * 1024 * 1024) / entry_size;

        // Round down to nearest power of 2 for efficient indexing
        let size = total_entries.next_power_of_two() / 2;
        let size_mask = size - 1;

        Self {
            entries: vec![TTEntry::default(); size],
            size_mask,
            generation: 0,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Increment generation for a new search
    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Get the index for a given zobrist key
    #[inline]
    fn get_index(&self, key: u64) -> usize {
        (key as usize) & self.size_mask
    }

    /// Probe the transposition table for a given position
    /// Returns the entry if found, regardless of depth (for move ordering)
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let index = self.get_index(key);
        let entry = &self.entries[index];

        // Check if this is a valid entry for our position
        if entry.key != key {
            self.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        self.hits.fetch_add(1, Ordering::Relaxed);
        Some(entry.clone())
    }

    /// Store an entry in the transposition table
    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        evaluation: f32,
        best_move: BoardMove,
        node_type: NodeType,
    ) {
        let index = self.get_index(key);
        let existing = &self.entries[index];

        // Replacement strategy:
        // Always replace if:
        // 1. Empty slot (key == 0)
        // 2. Same position (always update with latest search)
        // 3. Old entry from previous search (different generation) AND new search is at least as deep

        let should_replace = existing.key == 0
            || existing.key == key
            || (existing.age != self.generation && depth >= existing.depth);

        if should_replace {
            self.entries[index] = TTEntry {
                key,
                depth,
                evaluation,
                best_move,
                node_type,
                age: self.generation,
            };
        }
    }

    /// Clear all entries in the table
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = TTEntry::default();
        }
        self.generation = 0;
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Get statistics about the table
    #[allow(dead_code)]
    pub fn get_stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }
}

// Extension trait for Game to work with transposition table
pub trait GameTranspositionExt {
    fn get_zobrist_key(&self) -> u64;
}

impl GameTranspositionExt for Game {
    fn get_zobrist_key(&self) -> u64 {
        self.zobrist_key
    }
}
