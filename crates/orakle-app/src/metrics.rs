use std::fs;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProcessRoleStats {
    pub count: usize,
    pub rss_kb: u64,
    pub pss_kb: u64,
    pub swap_kb: u64,
    pub cpu_ms: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CgroupMemoryBreakdown {
    pub memory_current_kb: u64,
    pub anon_kb: u64,
    pub file_kb: u64,
    pub shmem_kb: u64,
    pub kernel_kb: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MemoryCpuSnapshot {
    pub main: ProcessRoleStats,
    pub webkit_web: ProcessRoleStats,
    pub webkit_network: ProcessRoleStats,
    pub webkit_gpu: ProcessRoleStats,
    pub other_children: ProcessRoleStats,
    pub total_rss_kb: u64,
    pub total_pss_kb: u64,
    pub total_swap_kb: u64,
    pub total_cpu_ms: u64,
    pub cgroup_memory_current_kb: Option<u64>,
    pub cgroup_breakdown: Option<CgroupMemoryBreakdown>,
}

fn clk_tck() -> u64 {
    #[cfg(target_os = "linux")]
    {
        100
    }
    #[cfg(not(target_os = "linux"))]
    {
        100
    }
}

fn read_pss_kb(pid: u32) -> u64 {
    let rollup = format!("/proc/{}/smaps_rollup", pid);
    if let Ok(content) = fs::read_to_string(rollup) {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("Pss:") {
                return rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);
            }
        }
    }
    0
}

fn read_cpu_ms(pid: u32) -> u64 {
    let stat_path = format!("/proc/{}/stat", pid);
    if let Ok(content) = fs::read_to_string(stat_path) {
        if let Some(rparen) = content.rfind(')') {
            let fields: Vec<&str> = content[rparen + 1..].split_whitespace().collect();
            if fields.len() > 12 {
                let utime: u64 = fields[11].parse().unwrap_or(0);
                let stime: u64 = fields[12].parse().unwrap_or(0);
                return ((utime + stime) * 1000) / clk_tck();
            }
        }
    }
    0
}

fn read_cgroup_dir() -> Option<String> {
    let cg = fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = cg.lines().next()?.split(':').nth(2)?.trim();
    Some(format!("/sys/fs/cgroup{}", rel))
}

pub fn read_cgroup_memory_current_kb() -> Option<u64> {
    let dir = read_cgroup_dir()?;
    let bytes: u64 = fs::read_to_string(format!("{}/memory.current", dir))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(bytes / 1024)
}

pub fn read_cgroup_breakdown() -> Option<CgroupMemoryBreakdown> {
    let dir = read_cgroup_dir()?;
    let current = read_cgroup_memory_current_kb()?;
    let stat = fs::read_to_string(format!("{}/memory.stat", dir)).ok()?;
    let mut b = CgroupMemoryBreakdown {
        memory_current_kb: current,
        ..Default::default()
    };
    for line in stat.lines() {
        let mut parts = line.split_whitespace();
        let k = parts.next().unwrap_or("");
        let v: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        match k {
            "anon" => b.anon_kb = v / 1024,
            "file" => b.file_kb = v / 1024,
            "shmem" => b.shmem_kb = v / 1024,
            "kernel" => b.kernel_kb = v / 1024,
            _ => {}
        }
    }
    Some(b)
}

pub fn capture_memory_cpu_snapshot() -> MemoryCpuSnapshot {
    let my_pid = std::process::id();
    let breakdown = read_cgroup_breakdown();
    let mut snap = MemoryCpuSnapshot {
        cgroup_memory_current_kb: breakdown
            .as_ref()
            .map(|b| b.memory_current_kb)
            .or_else(read_cgroup_memory_current_kb),
        cgroup_breakdown: breakdown,
        ..Default::default()
    };

    if let Ok(entries) = fs::read_dir("/proc") {
        let mut procs: Vec<(u32, u32, String, u64, u64)> = Vec::new();
        for entry in entries.flatten() {
            if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                let status_path = format!("/proc/{}/status", pid);
                if let Ok(status) = fs::read_to_string(status_path) {
                    let mut ppid = 0u32;
                    let mut rss = 0u64;
                    let mut swap = 0u64;
                    let mut comm = String::new();
                    for line in status.lines() {
                        if let Some(rest) = line.strip_prefix("Name:") {
                            comm = rest.trim().to_string();
                        } else if let Some(rest) = line.strip_prefix("PPid:") {
                            ppid = rest.trim().parse().unwrap_or(0);
                        } else if let Some(rest) = line.strip_prefix("VmRSS:") {
                            rss = rest
                                .split_whitespace()
                                .next()
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                        } else if let Some(rest) = line.strip_prefix("VmSwap:") {
                            swap = rest
                                .split_whitespace()
                                .next()
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                        }
                    }
                    procs.push((pid, ppid, comm, rss, swap));
                }
            }
        }

        let mut tree_pids = vec![my_pid];
        let mut changed = true;
        while changed {
            changed = false;
            for (pid, ppid, _, _, _) in &procs {
                if tree_pids.contains(ppid) && !tree_pids.contains(pid) {
                    tree_pids.push(*pid);
                    changed = true;
                }
            }
        }

        for (pid, _, comm, rss, swap) in procs {
            if !tree_pids.contains(&pid) {
                continue;
            }
            let pss = read_pss_kb(pid);
            let cpu = read_cpu_ms(pid);
            snap.total_rss_kb += rss;
            snap.total_pss_kb += pss;
            snap.total_swap_kb += swap;
            snap.total_cpu_ms += cpu;

            let target = if pid == my_pid {
                &mut snap.main
            } else if comm.contains("WebKitWeb") {
                &mut snap.webkit_web
            } else if comm.contains("WebKitNetwork") {
                &mut snap.webkit_network
            } else if comm.contains("WebKitGPU") || comm.contains("GPU") {
                &mut snap.webkit_gpu
            } else {
                &mut snap.other_children
            };
            target.count += 1;
            target.rss_kb += rss;
            target.pss_kb += pss;
            target.swap_kb += swap;
            target.cpu_ms += cpu;
        }
    }

    snap
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryPercentiles {
    pub min: f64,
    pub median: f64,
    pub p90: f64,
    pub p95: f64,
    pub max: f64,
}

fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let idx = ((n as f64) * p).ceil() as usize;
    sorted[idx.saturating_sub(1).min(n - 1)]
}

pub fn summarize(values: &[f64]) -> SummaryPercentiles {
    if values.is_empty() {
        return SummaryPercentiles {
            min: 0.0,
            median: 0.0,
            p90: 0.0,
            p95: 0.0,
            max: 0.0,
        };
    }
    let mut s = values.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    let median = if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    };
    SummaryPercentiles {
        min: s[0],
        median,
        p90: percentile_sorted(&s, 0.90),
        p95: percentile_sorted(&s, 0.95),
        max: s[n - 1],
    }
}
