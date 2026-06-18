//! Bridge engine: greedy approach + bitset DP exact gap-close.
//! Greedy gets within a small gap quickly, then bitset DP closes
//! the gap exactly using only the remaining (unused) elements.

use num_bigint::BigUint;
use num_traits::ToPrimitive;
use std::collections::HashSet;

use crate::bitset::Bitset;
use crate::controller::{Engine, Shared};

pub struct BridgeEngine;

const MAX_GAP_BITS: u64 = 24; // ~16 M gap

impl Engine for BridgeEngine {
    fn name(&self) -> &'static str {
        "Bridge"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        let mut desc_idx: Vec<usize> = (0..p.n).collect();
        desc_idx.sort_by(|&a, &b| p.numbers[b].cmp(&p.numbers[a]));

        for skip in 0..(5.min(p.n)) {
            if sh.stopped() {
                return;
            }
            let mut chosen: HashSet<usize> = HashSet::new();
            let mut total = BigUint::from(0u32);
            let mut sk = 0;
            for &i in &desc_idx {
                if sk < skip {
                    sk += 1;
                    continue;
                }
                let candidate = &total + &p.numbers[i];
                if candidate <= p.target {
                    chosen.insert(i);
                    total = candidate;
                    if total == p.target {
                        let sol: Vec<BigUint> =
                            chosen.iter().map(|&i| p.numbers[i].clone()).collect();
                        sh.report(sol, "Bridge");
                        return;
                    }
                }
            }

            if total == p.target {
                let sol: Vec<BigUint> =
                    chosen.iter().map(|&i| p.numbers[i].clone()).collect();
                sh.report(sol, "Bridge");
                return;
            }
            let gap = &p.target - &total;
            if gap.bits() > MAX_GAP_BITS {
                continue;
            }
            let gap_usize = match gap.to_usize() {
                Some(g) => g,
                None => continue,
            };

            let remaining: Vec<usize> = (0..p.n)
                .filter(|i| !chosen.contains(i))
                .filter_map(|i| p.numbers[i].to_usize())
                .collect();
            if remaining.is_empty() {
                continue;
            }

            let mut dp = Bitset::new(gap_usize + 1);
            dp.set(0);
            let mut hist: Vec<Bitset> = Vec::with_capacity(remaining.len() + 1);
            hist.push(dp.clone());
            let mut found_at: Option<usize> = None;
            for (i, &num) in remaining.iter().enumerate() {
                if sh.stopped() {
                    return;
                }
                dp.shift_or_inplace(num);
                hist.push(dp.clone());
                if dp.get(gap_usize) {
                    found_at = Some(i);
                    break;
                }
            }
            if let Some(_pos) = found_at {
                let mut cur = gap_usize;
                let mut sub: Vec<usize> = Vec::new();
                for i in (0..remaining.len()).rev() {
                    if cur == 0 {
                        break;
                    }
                    if i + 1 >= hist.len() {
                        continue;
                    }
                    let v = remaining[i];
                    if cur >= v && hist[i].get(cur - v) {
                        sub.push(v);
                        cur -= v;
                    }
                }
                if cur == 0 {
                    let mut sol: Vec<BigUint> =
                        chosen.iter().map(|&i| p.numbers[i].clone()).collect();
                    for v in sub {
                        sol.push(BigUint::from(v));
                    }
                    sh.report(sol, "Bridge");
                    return;
                }
            }
        }
    }
}
