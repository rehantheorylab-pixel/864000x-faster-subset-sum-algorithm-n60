//! Z++ Ultimate Engine — Rust Edition
//! Algorithm Design & Research: Rehan (Independent Researcher)
//!
//! Modes:
//!   1. Demo solve (small built-in instance)
//!   2. Headless: read elements + goal from stdin, report timing
//!
//! Timing precision: wall-clock via `Instant` (QueryPerformanceCounter
//! on Windows, ~100 ns), CPU time via `cpu_time` crate.  Display
//! decomposed down to attoseconds (informational; physical resolution
//! is bounded by the OS timer, typically 100 ns on Windows).

mod bitset;
mod controller;
mod engines;
mod gdvs;
mod gpu;
mod knapsack;
mod learning;
mod preprocess;
mod profile;
mod structure;
mod timing;
mod trivial;

use cpu_time::ProcessTime;
use num_bigint::BigUint;
use num_traits::Zero;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

use controller::{pick_engines, race, Engine, Outcome};
use learning::LearningStore;
use preprocess::reduce;
use profile::Profile;
use trivial::{solve as trivial_solve, Trivial};

fn main() {
    println!();
    println!("  Z++ Ultimate Engine — Rust Edition (v1.1)");
    println!("  Select Run Mode:");
    println!("    [1] Demo Mode (built-in instance)");
    println!("    [2] Headless Mode (comma-separated elements + goal)");
    println!("    [3] Load from file (e.g. z_test_elements.txt)");
    println!();
    print!("  Enter choice (1, 2, or 3): ");
    let _ = io::stdout().flush();

    let mut line = String::new();
    let stdin = io::stdin();
    stdin.lock().read_line(&mut line).ok();
    let choice = line.trim();

    match choice {
        "2" => run_headless(),
        "3" => run_file(),
        _ => run_demo(),
    }
}

fn run_demo() {
    let nums = vec![
        1u64, 3, 7, 21, 50, 200, 400, 499, 1000, 1500, 2000, 5000, 10000, 25000,
    ];
    let target = 5570u64;
    let nums_big: Vec<BigUint> = nums.iter().map(|n| BigUint::from(*n)).collect();
    solve_and_report(nums_big, BigUint::from(target));
}

fn run_file() {
    use std::path::Path;

    println!();
    println!("{}", "=".repeat(56));
    println!("   Z++ FILE LOAD MODE");
    println!("{}", "=".repeat(56));
    println!();
    println!("  Enter path to .txt file");
    println!("  (comma-separated elements, then line: goal: NUMBER)");
    print!("  Path: ");
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let mut path_line = String::new();
    stdin.lock().read_line(&mut path_line).ok();
    let path = path_line.trim().trim_matches('"');

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let default = format!("{}/../jnh1.cnf/z_test_elements.txt", manifest_dir);
    let path = if path.is_empty() { default } else { path.to_string() };

    if !Path::new(&path).exists() {
        println!("  File not found: {}", path);
        return;
    }

    println!("  Reading {} ...", path);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            println!("  Read error: {}", e);
            return;
        }
    };

    let (nums, target) = match parse_file_content(&content) {
        Some(p) => p,
        None => {
            println!("  Could not parse file. Expected:");
            println!("    elem1, elem2, ...");
            println!("    goal: 12345");
            return;
        }
    };

    println!("  Loaded {} elements", nums.len());
    let td = target.to_str_radix(10).len();
    if td <= 40 {
        println!("  Target: {}", target);
    } else {
        println!("  Target: {}-digit number", td);
    }
    println!();
    solve_and_report(nums, target);
}

fn parse_file_content(content: &str) -> Option<(Vec<BigUint>, BigUint)> {
    let goal_marker = "\ngoal:";
    let (elem_part, goal_part) = if let Some(idx) = content.find(goal_marker) {
        (&content[..idx], &content[idx + goal_marker.len()..])
    } else if let Some(idx) = content.rfind("goal:") {
        let before = &content[..idx];
        let after = &content[idx + 5..];
        (before, after)
    } else {
        return None;
    };

    let nums: Vec<BigUint> = elem_part
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| BigUint::parse_bytes(s.as_bytes(), 10))
        .collect();

    let goal_digits: String = goal_part.chars().filter(|c| c.is_ascii_digit()).collect();
    let target = BigUint::parse_bytes(goal_digits.as_bytes(), 10)?;

    if nums.is_empty() {
        return None;
    }
    Some((nums, target))
}

fn run_headless() {
    println!();
    println!("{}", "=".repeat(56));
    println!("   Z++ HEADLESS MODE");
    println!("{}", "=".repeat(56));
    println!();
    println!("  Enter elements (comma-separated):");
    print!("  ");
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let mut elem_line = String::new();
    stdin.lock().read_line(&mut elem_line).ok();
    let nums: Vec<BigUint> = elem_line
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| BigUint::parse_bytes(s.as_bytes(), 10))
        .collect();

    print!("\n  Enter target goal: ");
    let _ = io::stdout().flush();
    let mut tgt_line = String::new();
    stdin.lock().read_line(&mut tgt_line).ok();
    let target = BigUint::parse_bytes(tgt_line.trim().as_bytes(), 10)
        .unwrap_or_else(BigUint::zero);

    if nums.is_empty() {
        println!("  (no elements provided)");
        return;
    }

    println!();
    println!("{}", "=".repeat(56));
    println!("   RUNNING Z++ ENGINE...");
    println!("{}", "=".repeat(56));
    println!("   Elements : {}", nums.len());
    let td = target.to_str_radix(10).len();
    if td <= 40 {
        println!("   Target   : {}", target);
    } else {
        println!("   Target   : {}-digit number", td);
    }
    println!("{}", "=".repeat(56));
    println!();

    solve_and_report(nums, target);
}

fn solve_and_report(numbers: Vec<BigUint>, target: BigUint) {
    let cpu_start = ProcessTime::now();
    let wall_start = Instant::now();

    let raw_target = target.clone();
    let n_input = numbers.len();

    // Quick trivial / preprocessing handling.
    let prof = Profile::new(numbers.clone(), target.clone());
    let triv = trivial_solve(&prof);
    let outcome: Outcome = match triv {
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
                Outcome {
                    solution: None,
                    winner: "IMPOSSIBLE",
                    proved_impossible: true,
                    wall: Duration::from_nanos(0),
                }
            } else if red.target.is_zero() {
                Outcome {
                    solution: Some(red.forced.clone()),
                    winner: "Preprocessor",
                    proved_impossible: false,
                    wall: Duration::from_nanos(0),
                }
            } else {
                let red_profile = Profile::new(red.numbers.clone(), red.target.clone());
                // Bit-size aware timeout — no u128_safe() gate.
                // Big integer inputs get the same aggressive engines as
                // u128 inputs, thanks to BigUint fallback paths.
                let timeout = if red_profile.looks_sat_encoded() {
                    Duration::from_secs(600)
                } else if red_profile.n > 100 {
                    Duration::from_secs(600)
                } else if red_profile.n > 80 {
                    Duration::from_secs(600)
                } else if red_profile.n > 68 {
                    Duration::from_secs(600)
                } else if red_profile.n > 64 {
                    Duration::from_secs(7200)
                } else if red_profile.n >= 44 {
                    Duration::from_secs(600)
                } else if red_profile.n > 60 {
                    Duration::from_secs(600)
                } else {
                    Duration::from_secs(300)
                };
                let mut learn = LearningStore::load();

                // SAT-encoded giants: ColumnSAT only first.
                // The u128_safe() gate is REMOVED — all inputs >= 44
                // get the full engine portfolio.  Big integers use
                // BigUint paths (linear time growth with bit length).
                let mut out = if red_profile.looks_sat_encoded() {
                    let sat_only: Vec<Box<dyn Engine>> =
                        vec![engines::build("ColumnSAT").unwrap()];
                    race(red_profile.clone(), sat_only, timeout)
                } else if red_profile.n >= 44 && red_profile.n <= 140 {
                    let names = controller::pick_engines(&red_profile);
                    let all: Vec<Box<dyn Engine>> = names
                        .iter()
                        .filter_map(|n| engines::build(n))
                        .collect();
                    race(red_profile.clone(), all, timeout)
                } else {
                    let names = controller::pick_engines(&red_profile);
                    let all: Vec<Box<dyn Engine>> = names
                        .iter()
                        .filter_map(|n| engines::build(n))
                        .collect();
                    race(red_profile.clone(), all, timeout)
                };

                if out.solution.is_none() && !out.proved_impossible {
                    let names = learn.bias_order(&red_profile, pick_engines(&red_profile));
                    let mut engines: Vec<Box<dyn Engine>> = Vec::new();
                    for name in names {
                        if let Some(e) = engines::build(name) {
                            engines.push(e);
                        }
                    }
                    out = race(red_profile.clone(), engines, timeout);
                }
                if out.winner != "Timeout" && out.winner != "IMPOSSIBLE" {
                    learn.record_win(&red_profile, out.winner);
                }
                if let Some(sol) = out.solution.as_mut() {
                    let mut full: Vec<BigUint> = red.forced.clone();
                    full.extend(sol.iter().cloned());
                    *sol = full;
                }
                out
            }
        }
    };

    let wall_end = wall_start.elapsed();
    let cpu_end = cpu_start.elapsed();

    print_report(outcome, raw_target, n_input, cpu_end, wall_end);
}

fn print_report(
    out: Outcome,
    target: BigUint,
    n_input: usize,
    cpu: Duration,
    wall: Duration,
) {
    let exact = match out.solution.as_ref() {
        Some(sol) => sol.iter().sum::<BigUint>() == target,
        None => false,
    };
    let td = target.to_str_radix(10).len();

    println!();
    println!("{}", "=".repeat(56));
    println!("   Z++ HEADLESS PERFORMANCE REPORT");
    println!("{}", "=".repeat(56));
    println!("   Match Found     : {}", exact);
    if out.proved_impossible {
        println!("   PROVED IMPOSSIBLE");
    }
    println!("   Engine Winner   : {}", out.winner);
    println!("   Input size      : {} elements", n_input);
    if let Some(sol) = out.solution.as_ref() {
        println!("   Solution Size   : {} elements", sol.len());
        if td <= 40 {
            let s_str: Vec<String> = sol.iter().map(|x| x.to_string()).collect();
            println!("   Solution        : [{}]", s_str.join(", "));
            let total: BigUint = sol.iter().sum();
            println!("   Sum             : {}", total);
        } else {
            let total: BigUint = sol.iter().sum();
            println!("   Sum verified    : {}", total == target);
        }
    }

    println!();
    println!("   --- WALL-CLOCK TIME (real elapsed) ---");
    println!("      {}", timing::fmt_duration(wall));
    println!();
    println!("   --- ACTIVE CPU TIME (work across all threads) ---");
    println!("      {}", timing::fmt_duration(cpu));
    println!();

    let par = timing::parallelism_ratio(cpu, wall);
    println!("   Parallelism ratio : {:.3}x", par);
    println!("     (>1.0 means real multi-core. Rust has NO GIL,");
    println!("      so this can exceed 1.0 — Python's GIL caps near 1.0.)");
    println!();
    println!("{}", "=".repeat(56));
    println!("   NOTE on sub-nanosecond display:");
    println!("     ns is the smallest physically meaningful unit on");
    println!("     consumer hardware (Windows QPC ~100 ns, Linux ~1 ns).");
    println!("     ps/fs/as columns are mathematical conversions of ns.");
    println!("{}", "=".repeat(56));
    println!();
}
