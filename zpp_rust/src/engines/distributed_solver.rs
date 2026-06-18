//! ASDE - Rehan Adaptive Super-Distributed Engine  
  
use num_bigint::BigUint;  
use num_traits::Zero;  
use std::collections::HashMap;  
use std::sync::Arc;  
  
use crate::controller::{Engine, Shared}; 
pub struct DistributedSolver;  
  

impl DistributedSolver {
    fn run_distributed(sh: &Shared) {
        let p = &sh.profile;
        if p.n < 10 || p.target.is_zero() { return; }
        let mpi_ws = std::env::var("ZPP_MPI_WORLD_SIZE").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
        let is_mpi = mpi_ws > 0 || std::env::var("OMPI_COMM_WORLD_SIZE").is_ok() || std::env::var("SLURM_NNODES").is_ok();
        if is_mpi { Self::mpi_sum_range(sh, mpi_ws.max(2)); }
        else {
            let nw = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4).min(64);
            Self::choose_and_run(sh, nw);
        }
    }

    fn choose_and_run(sh: &Shared, nw: usize) {
        let p = &sh.profile;
        if p.n >= 16 && p.n <= 50 && p.u128_safe() { Self::ss_quadrant_distributed(sh, nw); }
        else if p.n > 70 || !p.u128_safe() { Self::md_mitm_distributed(sh, nw); }
        else { Self::sum_range_partitioned(sh, nw); }
    }

    // ========== Strategy A: SS-Quadrant ==========
    fn ss_quadrant_distributed(sh: &Shared, nw: usize) {
        let p = &sh.profile;
        let q = p.n / 4;
        if q < 2 { return; }
        let lists: [Vec<BigUint>; 4] = [
            p.numbers[0..q].to_vec(), p.numbers[q..2*q].to_vec(),
            p.numbers[2*q..3*q].to_vec(), p.numbers[3*q..].to_vec(),
        ];
        let qs: Arc<Vec<HashMap<BigUint, Vec<BigUint>>>> = Arc::new(
            (0..4).map(|qi| {
                let list = &lists[qi]; let mut sums = HashMap::new();
                for mask in 0..(1usize << list.len()) {
                    if sh.stopped() { return sums; }
                    let mut sum = BigUint::zero();
                    let mut subset = Vec::new();
                    for i in 0..list.len() {
                        if mask & (1usize << i) != 0 { sum += &list[i]; subset.push(list[i].clone()); }
                    }
                    if sum <= p.target { sums.entry(sum).or_insert(subset); }
                }
                sums
            }).collect()
        );
        std::thread::scope(|_s| {
        for wid in 0..nw {
            let qs = Arc::clone(&qs);
            let tgt = p.target.clone();
            _s.spawn(move || {
                let q0k: Vec<&BigUint> = qs[0].keys().collect();
                let q1k: Vec<&BigUint> = qs[1].keys().collect();
                let total = q0k.len() * q1k.len();
                if total == 0 { return; }
                let chunk = (total + nw - 1) / nw;
                let start = wid * chunk;
                let end = (start + chunk).min(total);
                let mut idx = 0usize;
                for s0 in &q0k {
                    for s1 in &q1k {
                        if idx >= end { return; }
                        if idx >= start {
                            let sum01 = s0.clone() + s1.clone();
                            if sum01 <= tgt {
                                for s2 in qs[2].keys() {
                                    for s3 in qs[3].keys() {
                                        if &sum01 + s2 + s3 == tgt {
                                            let mut sol = Vec::new();
                                            let qs_ref = &*qs;
                                            if let Some(v0) = qs_ref[0].get(s0) { sol.extend(v0.iter().cloned()); }
                                            if let Some(v1) = qs_ref[1].get(s1) { sol.extend(v1.iter().cloned()); }
                                            if let Some(v2) = qs_ref[2].get(s2) { sol.extend(v2.iter().cloned()); }
                                            if let Some(v3) = qs_ref[3].get(s3) { sol.extend(v3.iter().cloned()); }
                                            sh.report(sol, "ASDE(SS-Quadrant)");
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        idx += 1;
                    }
                }
            });
        }
        });
    }

    // ========== Strategy B: MD-MITM distributed ==========
    fn md_mitm_distributed(sh: &Shared, nw: usize) {
        let mut sorted = sh.profile.numbers.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        let n = sorted.len();
        let ng = nw.min(n / 5 + 1).max(2);
        let cs = (n + ng - 1) / ng;
        let tgt = sh.profile.target.clone();
        std::thread::scope(|_s| {
        for g in 0..ng {
            let s = g * cs;
            let e = (s + cs).min(n);
            if s >= n { break; }
            let group: Vec<BigUint> = sorted[s..e].to_vec();
            let tgt = tgt.clone();
            _s.spawn(move || {
                let mut suff = vec![BigUint::zero(); group.len() + 1];
                for i in (0..group.len()).rev() { suff[i] = &group[i] + &suff[i + 1]; }
                let mut found = false; let mut res = Vec::new();
                Self::gdep_search2(&group, &suff, &tgt, 0, &BigUint::zero(), &[], &mut found, &mut res);
                if found { sh.report(res, "ASDE(MD-MITM)"); }
            });
        }
        });
    }

    // ========== Strategy C: Sum-Range Partitioned ==========
    fn sum_range_partitioned(sh: &Shared, nw: usize) {
        let nums = sh.profile.numbers.clone();
        let tgt = sh.profile.target.clone();
        std::thread::scope(|_s| {
        for wid in 0..nw {
            let nums = nums.clone(); let tgt = tgt.clone();
            _s.spawn(move || {
                let mut local: Vec<BigUint> = nums.iter().enumerate().filter(|(i, _)| i % nw == wid).map(|(_, v)| v.clone()).collect();
                if local.is_empty() { return; }
                local.sort_by(|a, b| b.cmp(a));
                let mut suff = vec![BigUint::zero(); local.len() + 1];
                for i in (0..local.len()).rev() { suff[i] = &local[i] + &suff[i + 1]; }
                let mut found = false; let mut res = Vec::new();
                Self::gdep_search2(&local, &suff, &tgt, 0, &BigUint::zero(), &[], &mut found, &mut res);
                if found { sh.report(res, "ASDE(SumRange)"); }
            });
        }
        });
    }

    // ========== MPI Mode ==========
    fn mpi_sum_range(sh: &Shared, ws: usize) {
        let nums = sh.profile.numbers.clone();
        let tgt = sh.profile.target.clone();
        std::thread::scope(|_s| {
        for rank in 0..ws {
            let nums = nums.clone(); let tgt = tgt.clone();
            _s.spawn(move || {
                let mut local: Vec<BigUint> = nums.iter().enumerate().filter(|(i, _)| i % ws == rank).map(|(_, v)| v.clone()).collect();
                if local.is_empty() { return; }
                local.sort_by(|a, b| b.cmp(a));
                let mut suff = vec![BigUint::zero(); local.len() + 1];
                for i in (0..local.len()).rev() { suff[i] = &local[i] + &suff[i + 1]; }
                let mut found = false; let mut res = Vec::new();
                Self::gdep_search2(&local, &suff, &tgt, 0, &BigUint::zero(), &[], &mut found, &mut res);
                if found { sh.report(res, "ASDE(MPI)"); }
            });
        }
        });
    }
    fn gdep_search2(e: &[BigUint], suff: &[BigUint], tgt: &BigUint, start: usize, cur: &BigUint, path: &[BigUint], found: &mut bool, res: &mut Vec<BigUint>) {
        if *found { return; }
        if cur == tgt { *found = true; *res = path.to_vec(); return; }
        if start >= e.len() { return; }
        if &suff[start] + cur < *tgt { return; }
        for i in start..e.len() {
            if *found { return; }
            let nsum = cur + &e[i];
            if nsum > *tgt { continue; }
            if &suff[i] + cur < *tgt { break; }
            let mut np = path.to_vec();
            np.push(e[i].clone());
            Self::gdep_search2(e, suff, tgt, i + 1, &nsum, &np, found, res);
        }
    }
}

impl Engine for DistributedSolver {
    fn name(&self) -> &'static str { "DistributedSolver" }
    fn run(&self, sh: &Shared) { Self::run_distributed(sh); }
}
