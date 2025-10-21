use crate::game::board::BoardMove;
use std::sync::atomic::{AtomicU64, Ordering};

const BUCKET_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Debug, Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub depth: u8,
    pub evaluation: f32,
    pub best_move: BoardMove,
    pub node_type: NodeType,
    pub age: u8,
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

impl TTEntry {
    fn replacement_score(&self, current_generation: u8) -> i32 {
        if self.key == 0 {
            return -1000;
        }

        let depth_score = self.depth as i32 * 8;

        let age_diff = current_generation.wrapping_sub(self.age);
        let age_penalty = (age_diff as i32).min(15) * 3;

        let node_type_bonus = match self.node_type {
            NodeType::Exact => 25,     // PV nodes most valuable
            NodeType::LowerBound => 5, // Cut nodes somewhat valuable
            NodeType::UpperBound => 0, // All nodes least valuable
        };

        depth_score + node_type_bonus - age_penalty
    }
}

pub struct TranspositionTable {
    buckets: Vec<[TTEntry; BUCKET_SIZE]>,
    bucket_count: usize,
    generation: u8,
    hits: AtomicU64,
    misses: AtomicU64,
    filled_entries: AtomicU64,
    overwrites: AtomicU64,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>() * BUCKET_SIZE;
        let total_buckets = (size_mb * 1024 * 1024) / entry_size;

        let bucket_count = total_buckets.min(total_buckets.next_power_of_two());

        Self {
            buckets: vec![[TTEntry::default(); BUCKET_SIZE]; bucket_count],
            bucket_count,
            generation: 0,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            filled_entries: AtomicU64::new(0),
            overwrites: AtomicU64::new(0),
        }
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    fn get_bucket_index(&self, key: u64) -> usize {
        (key as usize) % self.bucket_count
    }

    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let bucket_idx = self.get_bucket_index(key);
        let bucket = &self.buckets[bucket_idx];

        for entry in bucket.iter() {
            if entry.key == key {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(*entry);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        evaluation: f32,
        best_move: BoardMove,
        node_type: NodeType,
    ) {
        let bucket_idx = self.get_bucket_index(key);
        let bucket = &mut self.buckets[bucket_idx];

        let new_entry = TTEntry {
            key,
            depth,
            evaluation,
            best_move,
            node_type,
            age: self.generation,
        };

        // First pass: look for same position or empty slot
        for i in 0..BUCKET_SIZE {
            if bucket[i].key == key {
                // Replace if: newer generation, OR (same generation AND deeper/equal depth)
                let is_newer = self.generation.wrapping_sub(bucket[i].age) > 0;
                if is_newer || depth >= bucket[i].depth {
                    bucket[i] = new_entry;
                }
                return;
            }
            if bucket[i].key == 0 {
                bucket[i] = new_entry;
                self.filled_entries.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // Second pass: find entry to replace based on replacement score
        let mut worst_idx = 0;
        let mut worst_score = i32::MAX;

        for i in 0..BUCKET_SIZE {
            let score = bucket[i].replacement_score(self.generation);
            if score < worst_score {
                worst_score = score;
                worst_idx = i;
            }
        }

        bucket[worst_idx] = new_entry;
        self.overwrites.fetch_add(1, Ordering::Relaxed);
    }

    pub fn prune_old_entries(&mut self) -> usize {
        const MAX_AGE_DIFF: u8 = 2;
        let mut pruned = 0u64;

        for bucket in self.buckets.iter_mut() {
            for entry in bucket.iter_mut() {
                if entry.key != 0 {
                    let age_diff = self.generation.wrapping_sub(entry.age);
                    if age_diff > MAX_AGE_DIFF {
                        *entry = TTEntry::default();
                        pruned += 1;
                    }
                }
            }
        }

        if pruned > 0 {
            self.filled_entries.fetch_sub(pruned, Ordering::Relaxed);
        }

        return pruned as usize;
    }

    pub fn clear(&mut self) {
        for bucket in self.buckets.iter_mut() {
            *bucket = [TTEntry::default(); BUCKET_SIZE];
        }

        self.generation = 0;
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.filled_entries.store(0, Ordering::Relaxed);
        self.overwrites.store(0, Ordering::Relaxed);
    }

    pub fn get_fullness_permille(&self) -> u64 {
        let filled = self.filled_entries.load(Ordering::Relaxed);
        let total_slots = (self.bucket_count * BUCKET_SIZE) as u64;

        if total_slots == 0 {
            0
        } else {
            (filled * 1000) / total_slots
        }
    }

    pub fn get_hit_rate_percent(&self) -> u64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 { 0 } else { (hits * 100) / total }
    }
}
