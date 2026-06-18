//! K-Sum engine: hash-accelerated lookup for solutions of size 1, 2,
//! 3, or 4.  Runs O(n^2) in the worst case (the 4-Sum stage).

use num_bigint::BigUint;
use std::collections::HashMap;

use crate::controller::{Engine, Shared};

pub struct KSumEngine;

impl Engine for KSumEngine {
    fn name(&self) -> &'static str {
        "KSum"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;

        // 3-Sum
        for i in 0..p.n {
            if sh.stopped() {
                return;
            }
            for j in (i + 1)..p.n {
                let two = &p.numbers[i] + &p.numbers[j];
                if two >= p.target {
                    continue;
                }
                let c = &p.target - &two;
                if !p.freq.contains_key(&c) {
                    continue;
                }
                let mut needed: HashMap<&BigUint, u32> = HashMap::new();
                *needed.entry(&p.numbers[i]).or_insert(0) += 1;
                *needed.entry(&p.numbers[j]).or_insert(0) += 1;
                *needed.entry(&c).or_insert(0) += 1;
                if needed
                    .iter()
                    .all(|(v, &cnt)| p.freq.get(*v).copied().unwrap_or(0) >= cnt)
                {
                    let mut sol = vec![p.numbers[i].clone(), p.numbers[j].clone(), c.clone()];
                    sol.sort();
                    sh.report(sol, "KSum");
                    return;
                }
            }
        }

        if p.n > 300 {
            return;
        }

        // 4-Sum via 2-Sum of pair sums.
        let mut pairs: HashMap<BigUint, (usize, usize)> = HashMap::new();
        for i in 0..p.n {
            if sh.stopped() {
                return;
            }
            for j in (i + 1)..p.n {
                let s = &p.numbers[i] + &p.numbers[j];
                if s >= p.target {
                    continue;
                }
                let comp = &p.target - &s;
                if let Some(&(pi, pj)) = pairs.get(&comp) {
                    if pi != i && pi != j && pj != i && pj != j {
                        let vals = [
                            p.numbers[pi].clone(),
                            p.numbers[pj].clone(),
                            p.numbers[i].clone(),
                            p.numbers[j].clone(),
                        ];
                        let mut needed: HashMap<&BigUint, u32> = HashMap::new();
                        for v in &vals {
                            *needed.entry(v).or_insert(0) += 1;
                        }
                        if needed
                            .iter()
                            .all(|(v, &cnt)| p.freq.get(*v).copied().unwrap_or(0) >= cnt)
                        {
                            let mut sol: Vec<BigUint> = vals.into_iter().collect();
                            sol.sort();
                            sh.report(sol, "KSum");
                            return;
                        }
                    }
                }
                pairs.entry(s).or_insert((i, j));
            }
        }
    }
}
