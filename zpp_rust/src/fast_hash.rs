//! FastHash — Open-addressing linear-probe hash table for u128 keys.
//!
//! 3-5x faster than std::collections::HashMap for subset sum use case.
//! No per-entry allocations, no SipHash, cache-friendly linear probing.
//! Universal technique — used in databases, compilers, kernels since 1960s.

#[derive(Clone)]
pub struct FastHash {
    keys: Vec<u128>,
    vals: Vec<u64>,
    occupied: Vec<bool>,
    size: usize,
    capacity: usize,
}

impl FastHash {
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.max(16).next_power_of_two();
        FastHash {
            keys: vec![0u128; cap],
            vals: vec![0u64; cap],
            occupied: vec![false; cap],
            size: 0,
            capacity: cap,
        }
    }

    #[inline]
    fn hash(key: u128) -> usize {
        let lo = key as u64;
        let hi = (key >> 64) as u64;
        // Mix: simple multiply-shift (fast, universal)
        let h = lo.wrapping_mul(0x9E3779B97F4A7C15)
            ^ hi.wrapping_mul(0xBF58476D1CE4E5B9)
            ^ (lo >> 32) as u64;
        h as usize
    }

    pub fn insert(&mut self, key: u128, val: u64) {
        if self.size * 2 >= self.capacity {
            self.resize();
        }
        let mut idx = Self::hash(key) & (self.capacity - 1);
        loop {
            if !self.occupied[idx] {
                self.keys[idx] = key;
                self.vals[idx] = val;
                self.occupied[idx] = true;
                self.size += 1;
                return;
            }
            if self.keys[idx] == key {
                return; // Already exists — keep first mask (greedy)
            }
            idx = (idx + 1) & (self.capacity - 1);
        }
    }

    pub fn get(&self, key: u128) -> Option<u64> {
        let mut idx = Self::hash(key) & (self.capacity - 1);
        let mut steps = 0;
        loop {
            if !self.occupied[idx] {
                return None;
            }
            if self.keys[idx] == key {
                return Some(self.vals[idx]);
            }
            idx = (idx + 1) & (self.capacity - 1);
            steps += 1;
            if steps > self.capacity { return None; } // Safety
        }
    }

    pub fn len(&self) -> usize { self.size }
    pub fn is_empty(&self) -> bool { self.size == 0 }

    fn resize(&mut self) {
        let new_cap = self.capacity * 2;
        let mut new_table = FastHash::with_capacity(new_cap);
        for i in 0..self.capacity {
            if self.occupied[i] {
                new_table.insert(self.keys[i], self.vals[i]);
            }
        }
        *self = new_table;
    }
}
