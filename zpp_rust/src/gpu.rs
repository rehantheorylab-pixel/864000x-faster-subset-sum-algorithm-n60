//! GPU detection and acceleration for Z++.
//!
//! ## Why this matters
//!
//! A modern GPU has thousands of cores (RTX 4090: 16384 CUDA cores,
//! RX 7900 XTX: 6144 stream processors).  If we can split the
//! subset-sum search space across ALL of them, we get near-linear
//! speedup with the partition count.
//!
//! ## Detection (runs once, cached forever)
//!
//! At first startup we probe for NVIDIA CUDA (`nvidia-smi`), AMD ROCm
//! (`rocm-smi`), and generic OpenCL (`clinfo`).  The result is saved
//! to `<CONFIG_DIR>/zpp_gpu.txt` so subsequent launches skip detection.
//!
//! ## Partitioning strategy
//!
//! GPU partitions use the same sum-range slicing as the CPU parallel
//! Schroeppel–Shamir (see knapsack.rs) but each slot is sized to
//! keep every GPU thread busy.  The host CPU generates the sorted
//! subset-sum arrays for each quarter, uploads them to GPU memory,
//! and each GPU thread walks one AB/CD slice.
//!
//! ## Current status
//!
//! Detection: ✅ implemented
//! Cache: ✅ implemented
//! GPU compute kernel: ⚡ planned (CUDA / WGSL compute shader)
//! Fallback: when no GPU is found, all partitions run on CPU cores.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// GPU capability summary detected at startup.
#[derive(Clone, Debug)]
pub struct GpuInfo {
    /// Number of GPU compute units found (0 = no GPU)
    pub compute_units: u32,
    /// Human-readable GPU name, e.g. "NVIDIA GeForce RTX 4090"
    pub name: String,
    /// Detected backend: "cuda", "rocm", "opencl", or "none"
    pub backend: String,
    /// GPU memory in bytes (0 = unknown)
    pub memory_bytes: u64,
}

static GPU_CACHE: OnceLock<GpuInfo> = OnceLock::new();

const CACHE_FILE: &str = "zpp_gpu.txt";

/// Returns detected GPU info, performing detection on first call.
/// Subsequent calls return the cached result instantly.
pub fn detect() -> &'static GpuInfo {
    GPU_CACHE.get_or_init(|| {
        // Try loading cache first
        if let Some(info) = load_cache() {
            return info;
        }
        let info = detect_gpu_once();
        cache_result(&info);
        info
    })
}

/// Force re-detection (ignore cache). Clears any stored result.
pub fn redetect() -> GpuInfo {
    let info = detect_gpu_once();
    cache_result(&info);
    // If the cache was already initialized, this is a no-op in the
    // current OnceLock usage, but we return the fresh result.
    info
}

/// The optimal number of CPU parallel work slots.
/// Currently returns CPU core count (the actual compute threads).
/// GPU compute units are stored for informational/display purposes and
/// for future GPU kernel work — once we implement WGSL/CUDA compute
/// shaders, the GPU's thousands of cores will each get a partition.
pub fn optimal_partition_count(cpu_cores: usize) -> usize {
    cpu_cores
}

/// Returns the detected GPU compute units (0 if no GPU).
pub fn gpu_compute_units() -> u32 {
    detect().compute_units
}

/// Try to detect a GPU by probing common tools.
fn detect_gpu_once() -> GpuInfo {
    // Try NVIDIA CUDA first
    if let Some(info) = probe_nvidia() {
        return info;
    }
    // Try AMD ROCm
    if let Some(info) = probe_amd() {
        return info;
    }
    // Try generic OpenCL
    if let Some(info) = probe_opencl() {
        return info;
    }
    // No GPU found
    GpuInfo {
        compute_units: 0,
        name: "No GPU detected".into(),
        backend: "none".into(),
        memory_bytes: 0,
    }
}

fn probe_nvidia() -> Option<GpuInfo> {
    let out = std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=name,memory.total,compute_cap")
        .arg("--format=csv,noheader")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 2 {
        return None;
    }
    let name = parts[0].trim().to_string();
    // Parse memory: e.g. "24576 MiB"
    let mem_text = parts[1].trim().split_whitespace().next().unwrap_or("0");
    let mem_mib: u64 = mem_text.parse().unwrap_or(0);
    let memory_bytes = mem_mib * 1024 * 1024;
    // Estimate compute units from memory size
    let compute_units = if mem_mib >= 24000 {
        16384 // RTX 4090 class
    } else if mem_mib >= 12000 {
        9728 // RTX 3080 class
    } else if mem_mib >= 8000 {
        6144 // RTX 3070 class
    } else if mem_mib >= 4000 {
        3584 // RTX 2060 class
    } else {
        1024 // entry level
    };
    Some(GpuInfo {
        compute_units,
        name,
        backend: "cuda".into(),
        memory_bytes,
    })
}

fn probe_amd() -> Option<GpuInfo> {
    let out = std::process::Command::new("rocm-smi")
        .arg("--showproductname")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let name = text.lines().find(|l| l.contains("Name")).unwrap_or("AMD GPU");
    Some(GpuInfo {
        compute_units: 6144,
        name: name.trim().to_string(),
        backend: "rocm".into(),
        memory_bytes: 0,
    })
}

fn probe_opencl() -> Option<GpuInfo> {
    let out = std::process::Command::new("clinfo")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(GpuInfo {
        compute_units: 4096,
        name: "OpenCL Device".into(),
        backend: "opencl".into(),
        memory_bytes: 0,
    })
}

fn cache_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("zpp").join(CACHE_FILE)
}

fn cache_result(info: &GpuInfo) {
    let path = cache_path();
    let line = format!(
        "{}|{}|{}|{}",
        info.backend, info.compute_units, info.memory_bytes, info.name
    );
    let _ = fs::create_dir_all(path.parent().unwrap());
    let _ = fs::write(&path, &line);
}

fn load_cache() -> Option<GpuInfo> {
    let path = cache_path();
    let content = fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.splitn(4, '|').collect();
    if parts.len() < 4 {
        return None;
    }
    let backend = parts[0].to_string();
    let compute_units: u32 = parts[1].parse().ok()?;
    let memory_bytes: u64 = parts[2].parse().ok()?;
    let name = parts[3].to_string();
    Some(GpuInfo {
        compute_units,
        name,
        backend,
        memory_bytes,
    })
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            compute_units: 0,
            name: String::new(),
            backend: String::new(),
            memory_bytes: 0,
        }
    }
}
