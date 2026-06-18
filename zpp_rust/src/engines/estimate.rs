//! Estimate Engine — your Algorithm A1 (mean-based estimation).
//!
//! Reference: `Subset sum algorithm.md` lines 1758, 21610.
//!
//! Steps (your refined formulation):
//!   1. Estimate k ≈ target / mean
//!   2. Try k_min..k_max bands (k-2 .. k+2) — the variance-aware band
//!   3. For each k, take the closest-to-mean elements and locally
//!      adjust by swapping in smaller / out larger to converge.
//!   4. Multi-start: try starting from largest, from median, from
//!      "elements closest to target / k" — three independent seeds.

use num_bigint::BigUint;
use num_traits::Zero;

use crate::controller::{Engine, Shared};

pub struct EstimateEngine;

const MAX_BAND: i32 = 3;
const ADJUST_ROUNDS: usize = 256;

impl Engine for EstimateEngine {
    fn name(&self) -> &'static str {
        "Estimate"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n == 0 {
            return;
        }
        let mean = if p.n == 0 {
            BigUint::from(1u32)
        } else {
            &p.total_sum / BigUint::from(p.n as u32)
        };
        if mean.is_zero() {
            return;
        }
        let k_estimate = (&p.target / &mean)
            .iter_u32_digits()
            .next()
            .unwrap_or(0) as i64;

        for delta in 0..=MAX_BAND {
            for sign in &[0i32, 1, -1] {
                if sh.stopped() {
                    return;
                }
                let k_signed = k_estimate + (*sign as i64) * (delta as i64);
                if k_signed < 1 || k_signed as usize > p.n {
                    continue;
                }
                let k = k_signed as usize;

                for seed in 0..3 {
                    if sh.stopped() {
                        return;
                    }
                    let chosen_idx = pick_initial(p, k, seed);
                    let mut sol = self.adjust(sh, p, chosen_idx);
                    if let Some(s) = sol.take() {
                        sh.report(s, "Estimate");
                        return;
                    }
                }
            }
        }
    }
}

impl EstimateEngine {
    fn adjust(
        &self,
        sh: &Shared,
        p: &crate::profile::Profile,
        mut chosen: Vec<usize>,
    ) -> Option<Vec<BigUint>> {
        let mut total: BigUint = chosen.iter().map(|&i| &p.numbers[i]).sum();
        if total == p.target {
            return Some(chosen.iter().map(|&i| p.numbers[i].clone()).collect());
        }
        for _ in 0..ADJUST_ROUNDS {
            if sh.stopped() {
                return None;
            }
            if total == p.target {
                return Some(chosen.iter().map(|&i| p.numbers[i].clone()).collect());
            }
            if total > p.target {
                let diff = &total - &p.target;
                let inside = chosen.iter().copied().collect::<std::collections::HashSet<_>>();
                let pick = chosen
                    .iter()
                    .copied()
                    .filter(|i| inside.contains(i))
                    .min_by_key(|i| absdiff_big(&p.numbers[*i], &diff));
                if let Some(out_i) = pick {
                    if let Some(pos) = chosen.iter().position(|&x| x == out_i) {
                        total -= &p.numbers[out_i];
                        chosen.remove(pos);
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                let need = &p.target - &total;
                let inside =
                    chosen.iter().copied().collect::<std::collections::HashSet<_>>();
                let cand = (0..p.n)
                    .filter(|i| !inside.contains(i))
                    .filter(|i| p.numbers[*i] <= need)
                    .min_by_key(|i| absdiff_big(&p.numbers[*i], &need));
                match cand {
                    Some(c) => {
                        total += &p.numbers[c];
                        chosen.push(c);
                    }
                    None => return None,
                }
            }
        }
        if total == p.target {
            Some(chosen.iter().map(|&i| p.numbers[i].clone()).collect())
        } else {
            None
        }
    }
}

fn pick_initial(p: &crate::profile::Profile, k: usize, seed: u32) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..p.n).collect();
    match seed {
        0 => idx.sort_by(|&a, &b| p.numbers[b].cmp(&p.numbers[a])), // largest first
        1 => {
            // closest to mean
            let mean = &p.total_sum / BigUint::from(p.n.max(1) as u32);
            idx.sort_by(|&a, &b| {
                absdiff_big(&p.numbers[a], &mean).cmp(&absdiff_big(&p.numbers[b], &mean))
            });
        }
        _ => {
            // closest to target/k — your A1 core
            if k > 0 {
                let target_per_k = &p.target / BigUint::from(k as u32);
                idx.sort_by(|&a, &b| {
                    absdiff_big(&p.numbers[a], &target_per_k)
                        .cmp(&absdiff_big(&p.numbers[b], &target_per_k))
                });
            }
        }
    }
    idx.into_iter().take(k).collect()
}

fn absdiff_big(a: &BigUint, b: &BigUint) -> BigUint {
    if a >= b { a - b } else { b - a }
}
