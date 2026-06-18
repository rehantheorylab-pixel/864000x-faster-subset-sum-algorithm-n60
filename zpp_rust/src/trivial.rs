//! Trivial / fast-path solver: handles the inputs that don't need
//! any heavy machinery.  Catches 1-Sum, 2-Sum, full-set, parity,
//! and multi-prime residue impossibility in O(n).

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{One, Zero};

use crate::profile::Profile;

const PRIMES: &[u32] = &[2, 3, 5, 7, 11, 13];

pub enum Trivial {
    Solved(Vec<BigUint>),
    Impossible,
    Continue,
}

pub fn solve(p: &Profile) -> Trivial {
    if p.target.is_zero() {
        return Trivial::Solved(vec![]);
    }
    if p.n == 0 {
        return Trivial::Impossible;
    }
    if p.target == p.total_sum {
        return Trivial::Solved(p.numbers.clone());
    }
    if p.target > p.total_sum {
        return Trivial::Impossible;
    }
    if p.min_val > p.target {
        return Trivial::Impossible;
    }
    if p.freq.contains_key(&p.target) {
        return Trivial::Solved(vec![p.target.clone()]);
    }

    // 2-Sum via O(n) hash lookup.
    for x in &p.numbers {
        if x >= &p.target {
            continue;
        }
        let c = &p.target - x;
        if let Some(&cnt) = p.freq.get(&c) {
            if c != *x || cnt >= 2 {
                let mut pair = vec![x.clone(), c];
                pair.sort();
                return Trivial::Solved(pair);
            }
        }
    }

    // Multi-prime residue feasibility.
    let two = BigUint::from(2u32);
    if (&p.target % &two).is_one() && p.numbers.iter().all(|x| (x % &two).is_zero()) {
        return Trivial::Impossible;
    }

    // GCD impossibility — if GCD(all elements) does not divide
    // the target, no subset can ever sum to the target.
    if !p.numbers.is_empty() {
        let mut g = p.numbers[0].clone();
        for x in &p.numbers[1..] {
            g = g.gcd(x);
            if g == BigUint::from(1u32) {
                break;
            }
        }
        if g > BigUint::from(1u32) && !(&p.target % &g).is_zero() {
            return Trivial::Impossible;
        }
    }

    for &prime in PRIMES {
        let p_big = BigUint::from(prime);
        let target_r = u32::try_from(&p.target % &p_big).unwrap_or(0);
        let mask: u64 = (1u64 << prime) - 1;
        let mut reach: u64 = 1;
        for x in &p.numbers {
            let r = u32::try_from(x % &p_big).unwrap_or(0);
            let shifted = reach << r;
            reach = (reach | shifted | (shifted >> prime)) & mask;
        }
        if (reach & (1u64 << target_r)) == 0 {
            return Trivial::Impossible;
        }
    }

    Trivial::Continue
}
