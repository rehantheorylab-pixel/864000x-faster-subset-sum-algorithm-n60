//! Z++ Meta-Brain controller - HARDWARE-ADAPTIVE VERSION
//!
//! Core design: detect hardware profile first, then select optimal engine
//! set for that hardware (desktop PC, server, supercomputer, or quantum).
//!
//! Rehan's thinking method applied:
//! 1. Decompose: classify problem type AND hardware type
//! 2. Estimate then adjust: pick engines that fit both
//! 3. Prune impossible: don't run engines that can't work on this hardware
//! 4. Multi-engine racing: run selected engines, first to find wins
//! 5. Memory-adaptive: don't use BitsetDP if RAM is tight
//!
//! ## Hardware-Adaptive Engine Selection
//!
//! - **Desktop** (1-64 cores, <=64GB RAM): balanced set, GPEP + Schroeppel-Shamir
//!   + BCJ/HGJ/Bonnetain for small n, MD-MITM for n>80
//! - **Server** (64-256 cores, large RAM): all engines, GPU-offloaded,
//!   massive BitsetDP, more aggressive partitioning
//! - **Supercomputer** (256+ cores, distributed, MPI): MPI-aware engines,
//!   distributed Schroeppel-Shamir and BCJ, massive parallel BitsetDP
//! - **Quantum**: Grover oracle formulation, hybrid quantum-classical bridge
use num_bigint::BigUint;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::hardware_profile::HardwareProfile;
use crate::profile::Profile;
use crate::structure::StructureInfo;
/// Shared state across engine threads.
pub struct Shared {
    pub profile: Profile,
    pub stop: AtomicBool,
    pub solution: Mutex<Option<(Vec<BigUint>, &'static str)>>,
    pub proved_impossible: AtomicBool,
    pub blackboard: Mutex<HashSet<BigUint>>,
    pub discovered_count: AtomicU64,
    pub signal: Condvar,
}

impl Shared {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            stop: AtomicBool::new(false),
            solution: Mutex::new(None),
            proved_impossible: AtomicBool::new(false),
            blackboard: Mutex::new(HashSet::with_capacity(1024)),
            discovered_count: AtomicU64::new(0),
            signal: Condvar::new(),
        }
    }
    #[inline]
    pub fn note_sum(&self, sum: BigUint) {
        if self.blackboard.lock().unwrap().insert(sum) {
            self.discovered_count.fetch_add(1, Ordering::Relaxed);
        }
    }
    /// Check whether a partial sum has already been discovered by any engine.
    /// Returns true if the sum is novel (not yet seen).
    #[inline]
    pub fn try_note_sum(&self, sum: &BigUint) -> bool {
        self.blackboard.lock().unwrap().insert(sum.clone())
    }
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn report(&self, sol: Vec<BigUint>, name: &'static str) {
        if self.stop.load(Ordering::Relaxed) { return; }
        let s: BigUint = sol.iter().sum();
        if s != self.profile.target { return; }
        let mut guard = self.solution.lock().unwrap();
        if guard.is_none() {
            *guard = Some((sol, name));
            self.stop.store(true, Ordering::Release);
            self.signal.notify_all();
        }
    }
}

pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, sh: &Shared);
}

pub struct Outcome {
    pub solution: Option<Vec<BigUint>>,
    pub winner: &'static str,
    pub proved_impossible: bool,
    pub wall: Duration,
}

pub fn race(profile: Profile, engines: Vec<Box<dyn Engine>>, max_time: Duration) -> Outcome {
    let shared = Arc::new(Shared::new(profile));
    let start = Instant::now();
    let ncpus = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let batch_size = (ncpus * 2).min(12);

    // Arc the engines so they can be shared across threads
    let engines = Arc::new(engines);
    let mut handles = Vec::with_capacity(engines.len());

    // Staggered launch: fast engines (high score) spawn first in small batches.
    // Heavy engines launch only if fast ones don't solve within 50ms.
    for batch_start in (0..engines.len()).step_by(batch_size) {
        if shared.stopped() || start.elapsed() >= max_time {
            break;
        }
        let batch_end = (batch_start + batch_size).min(engines.len());
        for idx in batch_start..batch_end {
            let sh = Arc::clone(&shared);
            let engines = Arc::clone(&engines);
            handles.push(thread::spawn(move || { engines[idx].run(&sh); }));
        }
        thread::sleep(Duration::from_millis(100)); // Give fast engines 100ms head start
    }
    // Wait on Condvar with timeout instead of busy-polling.
    let mut guard = shared.solution.lock().unwrap();
    let remaining = max_time.saturating_sub(start.elapsed());
    while !shared.stopped() && start.elapsed() < max_time {
        let (_guard, timeout) = shared.signal.wait_timeout(guard, remaining).unwrap();
        guard = _guard;
        if timeout.timed_out() || shared.stopped() {
            break;
        }
    }
    drop(guard);
    shared.stop.store(true, Ordering::Release);
    for h in handles { let _ = h.join(); }
    let wall = start.elapsed();
    let guard = shared.solution.lock().unwrap();
    let proved_impossible = shared.proved_impossible.load(Ordering::Relaxed);
    let (solution, winner) = match guard.clone() {
        Some((sol, name)) => (Some(sol), name),
        None => (None, if proved_impossible { "IMPOSSIBLE" } else { "Timeout" }),
    };
    Outcome { solution, winner, proved_impossible, wall }
}
/// Hardware-Adaptive Engine Selection
pub fn pick_engines(p: &Profile, hw: &HardwareProfile) -> Vec<&'static str> {
    // Detect structural patterns to guide engine selection.
    let struct_info = StructureInfo::detect(&p.numbers);
    let ordered = crate::scheduler::schedule(p, &struct_info, hw);
    if cfg!(debug_assertions) {
        // Verify scheduler output is a superset of core engines.
        let core = ["Residue", "DigitFilter", "Dominance"];
        for name in &core {
            debug_assert!(ordered.contains(name), "scheduler missing core engine: {}", name);
        }
    }
    ordered
}


