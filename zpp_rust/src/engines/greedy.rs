//! Greedy + feasibility-lookahead + recursive backtracking.
//! Largest-first selection with suffix-sum pruning so we never enter
//! a branch that cannot reach the target.

use num_bigint::BigUint;
use num_traits::Zero;

use crate::controller::{Engine, Shared};

pub struct GreedyEngine;

impl Engine for GreedyEngine {
    fn name(&self) -> &'static str {
        "Greedy"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        let mut desc = p.numbers.clone();
        desc.sort_by(|a, b| b.cmp(a));

        let n = desc.len();
        let mut suf: Vec<BigUint> = vec![BigUint::zero(); n + 1];
        for i in (0..n).rev() {
            suf[i] = &suf[i + 1] + &desc[i];
        }

        let mut path: Vec<BigUint> = Vec::new();
        let solved = solve(&desc, &suf, &p.target, 0, n, &mut path, sh);
        if solved {
            sh.report(path, "Greedy");
        }
    }
}

fn solve(
    nums: &[BigUint],
    suf: &[BigUint],
    target: &BigUint,
    start: usize,
    n: usize,
    path: &mut Vec<BigUint>,
    sh: &Shared,
) -> bool {
    if target.is_zero() {
        return true;
    }
    if start >= n || suf[start] < *target {
        return false;
    }
    if sh.stopped() {
        return false;
    }
    for i in start..n {
        let v = &nums[i];
        if v > target {
            continue;
        }
        if v == target {
            path.push(v.clone());
            return true;
        }
        let new_target = target - v;
        if suf[i + 1] >= new_target {
            path.push(v.clone());
            if solve(nums, suf, &new_target, i + 1, n, path, sh) {
                return true;
            }
            path.pop();
        }
    }
    false
}
