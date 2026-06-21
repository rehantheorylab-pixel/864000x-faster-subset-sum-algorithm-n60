pub mod bitset;
pub mod controller;
pub mod engines;
pub mod fast_hash;
pub mod fraction;
pub mod gdvs;
pub mod gpu;
pub mod gui;
pub mod hardware_profile;
pub mod knapsack;
pub mod learning;
pub mod preprocess;
pub mod profile;
pub mod scheduler;
pub mod settings;
pub mod structure;
pub mod timing;
pub mod trivial;

use num_bigint::BigUint;
use num_traits::Zero;
use std::time::Duration;

use controller::{pick_engines, race, Engine, Outcome};
use hardware_profile::HardwareProfile;
use learning::LearningStore;
use preprocess::reduce;
use profile::Profile;
use trivial::{solve as trivial_solve, Trivial};

pub fn solve(numbers: Vec<BigUint>, target: BigUint, timeout: Duration) -> Outcome {
    let hw = HardwareProfile::detect();
    let prof = Profile::new(numbers.clone(), target.clone());
    let triv = trivial_solve(&prof);
    match triv {
        Trivial::Solved(sol) => Outcome {
            solution: Some(sol),
            winner: "Trivial",
            proved_impossible: false,
            wall: Duration::from_nanos(0),
        },
        Trivial::Impossible => Outcome {
            solution: None,
            winner: "IMPOSSIBLE",
            proved_impossible: true,
            wall: Duration::from_nanos(0),
        },
        Trivial::Continue => {
            let red = reduce(&numbers, &target);
            if red.impossible {
                return Outcome {
                    solution: None,
                    winner: "IMPOSSIBLE",
                    proved_impossible: true,
                    wall: Duration::from_nanos(0),
                };
            }
            if red.target.is_zero() {
                return Outcome {
                    solution: Some(red.forced),
                    winner: "Preprocessor",
                    proved_impossible: false,
                    wall: Duration::from_nanos(0),
                };
            }
            let red_profile = Profile::new(red.numbers.clone(), red.target.clone());
            let names = pick_engines(&red_profile, &hw);
            let engines: Vec<Box<dyn Engine>> = names
                .iter()
                .filter_map(|n| engines::build(n))
                .collect();
            let mut out = race(red_profile.clone(), engines, timeout);
            if out.solution.is_none() && !out.proved_impossible {
                let learn = LearningStore::load();
                let names2 = learn.bias_order(&red_profile, pick_engines(&red_profile, &hw));
                let engines2: Vec<Box<dyn Engine>> = names2
                    .iter()
                    .filter_map(|n| engines::build(n))
                    .collect();
                out = race(red_profile.clone(), engines2, timeout);
            }
            if out.winner != "Timeout" && out.winner != "IMPOSSIBLE" {
                LearningStore::load().record_win(&red_profile, out.winner);
            }
            if let Some(sol) = out.solution.as_mut() {
                let mut full: Vec<BigUint> = red.forced.clone();
                full.extend(sol.iter().cloned());
                *sol = full;
            }
            out
        }
    }
}
