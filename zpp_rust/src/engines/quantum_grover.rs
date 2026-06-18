//! # QCC - Quantum-Classical CoProcessor
//! Hybrid: Z++ reduces n, Grover solves subproblems.

use num_bigint::BigUint;
use num_traits::Zero;
use crate::controller::{Engine, Shared};

pub struct QuantumGrover;
impl QuantumGrover {
    fn run_grover(sh: &Shared) {
        let p = &sh.profile; let n = p.n;
        if n > 30 { return; } // Cap to prevent exponential allocations
        if n > 35 { Self::hybrid_approach(sh, n, &p.numbers, &p.target); return; }
        let mut found = false; let mut solution = Vec::new();
        Self::simulate_grover(sh, n, &p.numbers, &p.target, &mut found, &mut solution);
        if found { sh.report(solution, "QuantumGrover"); }
    }
    fn hybrid_approach(sh: &Shared, n: usize, nums: &[BigUint], target: &BigUint) {
        let sub_size = 25.min(n); let ng = (n + sub_size - 1) / sub_size;
        let mut found = false; let mut solution = Vec::new();
        for g in 0..ng {
            if sh.stopped() { return; }
            let s = g * sub_size; let e = (s + sub_size).min(n);
            let group = &nums[s..e];
            let gs: BigUint = group.iter().sum();
            if gs <= *target {
                let mut sf = false; let mut ss = Vec::new();
                Self::simulate_grover(sh, group.len(), group, target, &mut sf, &mut ss);
                if sf { found = true; solution.extend(ss); }
            }
        }
        if found { sh.report(solution, "QuantumGrover(hybrid)"); }
    }
    fn simulate_grover(sh: &Shared, n: usize, nums: &[BigUint], target: &BigUint, found: &mut bool, solution: &mut Vec<BigUint>) {
        let limit = 1usize << n.min(30);
        for mask in 0..limit {
            if sh.stopped() { return; }
            let mut sum = BigUint::zero(); let mut subset = Vec::new();
            for i in 0..n { if mask & (1usize << i) != 0 { sum += &nums[i]; subset.push(nums[i].clone()); } }
            if sum == *target { *found = true; *solution = subset; return; }
        }
    }
}
impl Engine for QuantumGrover {
    fn name(&self) -> &'static str { "QuantumGrover" }
    fn run(&self, sh: &Shared) { Self::run_grover(sh); }
}
