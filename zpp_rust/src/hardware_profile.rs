//! # Hardware Profile — Adaptive Engine Selection
//!
//! **Core Idea (Rehan's thinking method applied):**
//! 1. Decompose: what resources does this machine have?
//! 2. Estimate then adjust: classify hardware, select optimal engine set
//! 3. Prune impossible: don't run engines that can't work within constraints
//! 4. Symmetry: if desktop approach fails, try distributed approach
//!
//! Detects CPU cores, RAM, GPU presence, and classifies the machine into:
//! - **Desktop**: 1-64 cores, limited RAM, no GPU or consumer GPU
//! - **Server**: 64-256 cores, large RAM, server GPU
//! - **Supercomputer**: 256-100K+ cores, distributed, MPI-capable
//! - **Quantum**: quantum computer (formulates Grover oracle)

/// Hardware classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HardwareClass {
    /// Desktop PC (1-64 cores, limited RAM)
    Desktop,
    /// Server / Workstation (64-256 cores, large RAM, possibly GPU)
    Server,
    /// Supercomputer / Cluster (256+ cores, distributed memory, MPI)
    Supercomputer,
    /// Quantum computer (formulates Grover oracle for QAOA/amplitude amplification)
    Quantum,
    /// Unknown — treat as desktop
    Unknown,
}

impl HardwareClass {
    pub fn name(&self) -> &'static str {
        match self {
            HardwareClass::Desktop => "Desktop",
            HardwareClass::Server => "Server",
            HardwareClass::Supercomputer => "Supercomputer",
            HardwareClass::Quantum => "Quantum",
            HardwareClass::Unknown => "Desktop",
        }
    }
}

/// Memory tier — how much RAM is available for DP tables
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryTier {
    /// < 2 GB — very constrained, avoid BitsetDP
    Tight,
    /// 2-8 GB — moderate, BitsetDP for small targets only
    Moderate,
    /// 8-64 GB — comfortable, full BitsetDP possible
    Generous,
    /// 64-512 GB — server-class, large DP tables
    Ample,
    /// 512+ GB — supercomputer-class, massive DP
    Massive,
}

/// Complete hardware profile
#[derive(Debug, Clone)]
pub struct HardwareProfile {
    /// Classification
    pub class: HardwareClass,
    /// Number of logical CPU cores
    pub cpu_cores: usize,
    /// Available physical RAM in bytes (approximate)
    pub available_ram: u64,
    /// Total physical RAM in bytes
    pub total_ram: u64,
    /// Memory tier
    pub memory_tier: MemoryTier,
    /// Whether a CUDA-capable GPU was detected
    pub has_gpu: bool,
    /// GPU compute capability (major * 10 + minor, e.g., 75 for sm_75)
    pub gpu_compute_cap: u32,
    /// Whether MPI-style distributed execution is available
    pub has_mpi: bool,
    /// Whether this is a quantum computer (hybrid quantum-classical)
    pub is_quantum: bool,
}

impl HardwareProfile {
    /// Detect hardware and build profile.
    pub fn detect() -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let total_ram = detect_total_ram();
        let available_ram = detect_available_ram();

        let has_gpu = detect_cuda_gpu();
        let compute_cap = detect_gpu_compute_capability();

        let memory_tier = classify_memory(available_ram);
        let class = classify_hardware(cpu_cores, total_ram, has_gpu);

        // MPI detection — check for mpi environment variable
        let has_mpi = std::env::var("OMPI_COMM_WORLD_SIZE")
            .or_else(|_| std::env::var("MPI_LOCALNRANKS"))
            .or_else(|_| std::env::var("SLURM_NNODES"))
            .map(|_| true)
            .unwrap_or(false);

        let is_quantum = std::env::var("QISKIT_SIM")
            .or_else(|_| std::env::var("QUBIT_SIMULATOR"))
            .map(|_| true)
            .unwrap_or(false);

        HardwareProfile {
            class,
            cpu_cores,
            available_ram,
            total_ram,
            memory_tier,
            has_gpu,
            gpu_compute_cap: compute_cap,
            has_mpi,
            is_quantum,
        }
    }

    /// Maximum BitsetDP size that fits in available RAM.
    /// BitsetDP needs target + 1 bits ≈ (target + 1) / 8 bytes.
    /// We reserve 80% of available RAM for the DP table.
    pub fn max_bitset_target(&self) -> u64 {
        let available_bytes = self.available_ram.saturating_sub(256 * 1024 * 1024); // Reserve 256MB for OS
        let usable = (available_bytes as f64 * 0.8) as u64;
        // Each bit = 1 integer, so max target = bytes * 8
        usable.saturating_mul(8)
    }

    /// Optimal thread count for Schroeppel-Shamir sum-range partitioning.
    /// Uses available cores, but caps at a reasonable maximum.
    pub fn optimal_partition_count(&self) -> usize {
        match self.class {
            HardwareClass::Desktop => self.cpu_cores.max(2),
            HardwareClass::Server => self.cpu_cores,
            HardwareClass::Supercomputer => {
                // For distributed: partition count = local cores * nodes
                let local = self.cpu_cores;
                let nodes = std::env::var("SLURM_NNODES")
                    .or_else(|_| std::env::var("OMPI_COMM_WORLD_SIZE"))
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);
                (local * nodes).min(1024)
            }
            HardwareClass::Quantum => 1, // Quantum does its own thing
            HardwareClass::Unknown => self.cpu_cores,
        }
    }

    /// Whether to enable GPU offload for supported engines.
    pub fn use_gpu(&self) -> bool {
        self.has_gpu && self.gpu_compute_cap >= 60 // sm_60 = Pascal or better
    }

    /// Whether to use the distributed (MPI) engine set.
    pub fn use_distributed(&self) -> bool {
        self.has_mpi || self.class == HardwareClass::Supercomputer
    }

    /// Whether to use the quantum formulation.
    pub fn use_quantum(&self) -> bool {
        self.is_quantum || self.class == HardwareClass::Quantum
    }
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn detect_total_ram() -> u64 {
    let info = match procfs::Meminfo::new() {
        Ok(m) => m,
        Err(_) => return 8 * 1024 * 1024 * 1024, // Assume 8 GB
    };
    info.mem_total as u64
}

#[cfg(not(target_os = "linux"))]
fn detect_total_ram() -> u64 {
    // Fallback: read from environment or assume reasonable default
    std::env::var("ZPP_TOTAL_RAM")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(12_169_000_000) // 12 GB — Rehan's machine
}

#[cfg(target_os = "linux")]
fn detect_available_ram() -> u64 {
    let info = match procfs::Meminfo::new() {
        Ok(m) => m,
        Err(_) => return 4 * 1024 * 1024 * 1024,
    };
    info.mem_available as u64
}

#[cfg(not(target_os = "linux"))]
fn detect_available_ram() -> u64 {
    std::env::var("ZPP_AVAILABLE_RAM")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5_269_000_000) // ~5 GB — Rehan's machine
}

fn detect_cuda_gpu() -> bool {
    // Try nvidia-smi (most reliable)
    let has_nvidia = std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=name")
        .arg("--format=csv,noheader")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_nvidia {
        return true;
    }

    // Try ROCm
    let has_rocm = std::process::Command::new("rocm-smi")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_rocm {
        return true;
    }

    // Try environment variable override
    std::env::var("ZPP_HAS_GPU")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .map(|v| v != 0)
        .unwrap_or(false)
}

fn detect_gpu_compute_capability() -> u32 {
    // Try nvidia-smi for compute capability
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let cap_str = stdout.trim();
            if let Some(dot) = cap_str.find('.') {
                let major: u32 = cap_str[..dot].parse().unwrap_or(0);
                let minor: u32 = cap_str[dot + 1..].parse().unwrap_or(0);
                return major * 10 + minor;
            }
        }
    }

    // Environment override
    std::env::var("ZPP_GPU_COMPUTE_CAP")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
}

fn classify_memory(available_ram: u64) -> MemoryTier {
    let gb = available_ram as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb < 2.0 {
        MemoryTier::Tight
    } else if gb < 8.0 {
        MemoryTier::Moderate
    } else if gb < 64.0 {
        MemoryTier::Generous
    } else if gb < 512.0 {
        MemoryTier::Ample
    } else {
        MemoryTier::Massive
    }
}

fn classify_hardware(cpu_cores: usize, total_ram: u64, has_gpu: bool) -> HardwareClass {
    // Environment variable override
    if let Ok(hw_class) = std::env::var("ZPP_HARDWARE_CLASS") {
        return match hw_class.to_lowercase().as_str() {
            "desktop" => HardwareClass::Desktop,
            "server" => HardwareClass::Server,
            "supercomputer" | "cluster" | "hpc" => HardwareClass::Supercomputer,
            "quantum" | "qubit" => HardwareClass::Quantum,
            _ => HardwareClass::Desktop,
        };
    }

    let ram_gb = total_ram as f64 / (1024.0 * 1024.0 * 1024.0);

    if cpu_cores >= 256 && ram_gb >= 512.0 {
        HardwareClass::Supercomputer
    } else if cpu_cores >= 64 && ram_gb >= 64.0 && has_gpu {
        HardwareClass::Server
    } else if cpu_cores >= 64 && ram_gb >= 128.0 {
        HardwareClass::Server
    } else {
        HardwareClass::Desktop
    }
}

// ---------------------------------------------------------------------------
// Engine selection per hardware class
// ---------------------------------------------------------------------------

/// Returns the list of engines to run for the current hardware profile.
///
/// This is Rehan's "pick based on problem type" logic, extended with
/// hardware awareness. Different engine sets for:
/// - Desktop PC (balanced, respects memory constraints)
/// - Supercomputer (distributed, MPI-based, massive parallelism)
/// - Quantum Computer (Grover oracle formulation)
/// - Server (GPU-enabled if available, maximum parallelism)
pub fn select_engines_for_hardware(
    hw: &HardwareProfile,
    n: usize,
    target_bits: u64,
    u128_safe: bool,
) -> Vec<&'static str> {
    let mut engines = match hw.class {
        HardwareClass::Desktop => {
            select_desktop_engines(hw, n, target_bits, u128_safe)
        }
        HardwareClass::Server => {
            select_server_engines(hw, n, target_bits, u128_safe)
        }
        HardwareClass::Supercomputer => {
            select_supercomputer_engines(hw, n, target_bits, u128_safe)
        }
        HardwareClass::Quantum => {
            select_quantum_engines(hw, n, target_bits, u128_safe)
        }
        HardwareClass::Unknown => {
            select_desktop_engines(hw, n, target_bits, u128_safe)
        }
    };
    // UnifiedSolver is the primary orchestrator — it runs ALL techniques
    // synergistically. Other engines are fast-path alternatives for cases
    // the UnifiedSolver might not optimize for (e.g., trivial instances).
    engines.insert(0, "UnifiedSolver");
    engines
}

fn select_desktop_engines(
    _hw: &HardwareProfile,
    n: usize,
    target_bits: u64,
    u128_safe: bool,
) -> Vec<&'static str> {
    let mut engines = Vec::new();

    // Always run: heuristic engines (fast, low overhead)
    engines.push("GDEP");
    engines.push("Greedy");
    engines.push("Bridge");
    engines.push("Decompose");

    // For small n: exact methods
    if n <= 30 {
        engines.push("MITM");
        engines.push("BitsetDP");
    }

    // For medium n: Schroeppel-Shamir (adaptive partitioning)
    if n <= 70 {
        engines.push("Schroeppel-Shamir");
    }

    // For hard instances: cryptanalytic engines
    if u128_safe {
        if n >= 40 && n <= 80 {
            engines.push("BCJ");
            engines.push("HGJ");
            engines.push("Bonnetain");
        }
    }

    // For large n (where other engines cap): MD-MITM
    if n > 70 || n >= 140 {
        engines.push("MD-MITM");
    }

    // Heuristic engines for all sizes
    if n > 20 {
        engines.push("APDE");
        engines.push("PMAS-Balance");
        engines.push("PMAS-Difference");
        engines.push("DualCollapse");
    }

    // Small target → BitsetDP
    if target_bits <= 24 {
        engines.push("BitsetDP");
    }

    // SAT-encoded detection is automatic in ColumnSAT engine

    engines
}

fn select_server_engines(
    hw: &HardwareProfile,
    n: usize,
    target_bits: u64,
    u128_safe: bool,
) -> Vec<&'static str> {
    let mut engines = select_desktop_engines(hw, n, target_bits, u128_safe);

    // Server has more cores, so add more parallel engines
    engines.push("GDEP");
    engines.push("Greedy");

    // GPU-offloaded engines if available
    if hw.use_gpu() {
        engines.push("Schroeppel-Shamir"); // GPU-assisted
    }

    // Larger BitsetDP possible on server
    if target_bits <= 28 {
        engines.push("BitsetDP");
    }

    // All cryptanalytic engines
    if u128_safe {
        engines.push("BCJ");
        engines.push("HGJ");
        engines.push("Bonnetain");
    }

    engines
}

fn select_supercomputer_engines(
    hw: &HardwareProfile,
    n: usize,
    _target_bits: u64,
    _u128_safe: bool,
) -> Vec<&'static str> {
    // On a supercomputer, we use distributed engines that scale
    // across nodes. These engines use MPI for inter-node communication
    // and are designed for 10,000+ core parallelism.
    let mut engines: Vec<&'static str> = Vec::new();

    // Core engines that scale to distributed memory
    engines.push("DistributedSolver"); // MPI-based distribution (all strategies)

    // Local engines (run on each node as well)
    engines.push("GDEP");
    engines.push("MD-MITM");
    engines.push("Greedy");
    engines.push("Bridge");

    // For large n: hierarchical decomposition across nodes
    if n > 80 {
        engines.push("MD-MITM");
        // MD-MITM's levels distribute naturally across nodes
    }

    // GPU-offloaded (each node's GPU)
    if hw.use_gpu() {
        engines.push("BitsetDP");    // GPU-accelerated variant
        engines.push("Schroeppel-Shamir");  // GPU-accelerated variant
    }

    engines
}

fn select_quantum_engines(
    _hw: &HardwareProfile,
    n: usize,
    _target_bits: u64,
    _u128_safe: bool,
) -> Vec<&'static str> {
    // Quantum computers use a formulation of subset sum as
    // a Grover oracle. The quantum engine produces the Grover
    // circuit / Hamiltonian, not a classical solution.
    let mut engines = Vec::new();

    engines.push("QuantumGrover");  // Grover oracle formulation + hybrid bridge

    // Also run classical engines for verification
    engines.push("GDEP");
    engines.push("Bridge");

    if n <= 70 {
        engines.push("Schroeppel-Shamir");
    }

    engines
}
