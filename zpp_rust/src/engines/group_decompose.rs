//! GroupDecompose — Hierarchical Decomposition Subset Sum Engine
//!
//! Core idea: split n elements into groups of ~10, compute all subset
//! sums per group (2^10=1024 max), then use depth-first search with
//! range pruning across groups. Complementary to 4-way Schroeppel-Shamir.

use num_bigint::BigUint;

use crate::controller::{Engine, Shared};

pub struct GroupDecomposeEngine;

const GD_MIN_N: usize = 28;
const GD_GROUP_SIZE: usize = 10;

impl Engine for GroupDecomposeEngine {
    fn name(&self) -> &'static str {
        "GroupDecompose"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < GD_MIN_N || !p.u128_safe() {
            return;
        }
        let target = p.target_u128();
        let nums = p.numbers_u128();
        let n = nums.len();
        let num_groups = (n + GD_GROUP_SIZE - 1) / GD_GROUP_SIZE;
        if num_groups < 3 || num_groups > 7 {
            return;
        }

        let mut group_sums: Vec<Vec<(u128, u64)>> = Vec::with_capacity(num_groups);
        let mut group_max = vec![0u128; num_groups];
        for g in 0..num_groups {
            let start = g * GD_GROUP_SIZE;
            let end = (start + GD_GROUP_SIZE).min(n);
            let sums = build_sums(&nums[start..end], target);
            if !sums.is_empty() {
                group_max[g] = sums.last().unwrap().0;
            }
            if sh.stopped() { return; }
            group_sums.push(sums);
        }

        // Precompute suffix max sums
        let mut suffix_max = vec![0u128; num_groups + 1];
        for g in (0..num_groups).rev() {
            suffix_max[g] = suffix_max[g + 1] + group_max[g];
        }

        // Depth-first search across groups
        let mut path: Vec<u64> = Vec::with_capacity(num_groups);
        if dfs(&group_sums, &suffix_max, 0, target, &mut path, sh) {
            let mut sol: Vec<BigUint> = Vec::new();
            for (g, mask) in path.iter().enumerate() {
                let start = g * GD_GROUP_SIZE;
                let end = (start + GD_GROUP_SIZE).min(n);
                let mut m = *mask;
                for i in 0..(end - start) {
                    if m & 1 != 0 {
                        sol.push(BigUint::from(nums[start + i]));
                    }
                    m >>= 1;
                }
            }
            sh.report(sol, "GroupDecompose");
        }
    }
}

fn build_sums(elems: &[u128], target: u128) -> Vec<(u128, u64)> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut sums = Vec::with_capacity(total as usize);
    let mut pref = vec![0u128; n + 1];
    for i in 0..n {
        pref[i + 1] = pref[i].wrapping_add(elems[i]);
    }
    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 {
            let k = mask.trailing_zeros() as usize;
            s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]);
        }
        if s <= target {
            sums.push((s, mask));
        }
    }
    sums.sort_unstable_by_key(|x| x.0);
    sums
}

fn dfs(
    groups: &[Vec<(u128, u64)>],
    suffix_max: &[u128],
    g: usize,
    remaining: u128,
    path: &mut Vec<u64>,
    sh: &Shared,
) -> bool {
    if g >= groups.len() {
        return remaining == 0;
    }
    if sh.stopped() {
        return false;
    }
    // Prune: remaining must be achievable by current + future groups
    let max_avail = suffix_max[g];
    if remaining > max_avail {
        return false;
    }

    let group = &groups[g];
    // Only try sums that don't exceed remaining
    let end = match group.binary_search_by(|e| e.0.cmp(&remaining)) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };

    // Try larger sums first (greedy — faster to find solutions for most instances)
    if end > 0 {
        // Binary search for start: remaining - suffix_max[g+1]
        let need = remaining.saturating_sub(suffix_max[g + 1]);
        let start = match group.binary_search_by(|e| e.0.cmp(&need)) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
        for j in (start..end).rev() {
            let (gs, mask) = group[j];
            if gs <= remaining {
                path.push(mask);
                if dfs(groups, suffix_max, g + 1, remaining - gs, path, sh) {
                    return true;
                }
                path.pop();
            }
        }
    }

    false
}
