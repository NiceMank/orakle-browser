use std::fs;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProcessRoleStats {
    pub count: usize,
    pub rss_kb: u64,
    pub pss_kb: u64,
    pub cpu_ms: u64,
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
    pub total_cpu_ms: u64,
    pub cgroup_memory_current_kb: Option<u64>,
}

fn clk_tck() -> u64 {
    #[cfg(target_os = "linux")]
    {
        // Standard Linux USER_HZ = 100
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
            // After ')', field 0 is state, utime is field 11, stime is field 12
            if fields.len() > 12 {
                let utime: u64 = fields[11].parse().unwrap_or(0);
                let stime: u64 = fields[12].parse().unwrap_or(0);
                return ((utime + stime) * 1000) / clk_tck();
            }
        }
    }
    0
}

pub fn read_cgroup_memory_current_kb() -> Option<u64> {
    let cg = fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = cg.lines().next()?.split(':').nth(2)?.trim();
    let full = format!("/sys/fs/cgroup{}/memory.current", rel);
    let bytes: u64 = fs::read_to_string(full).ok()?.trim().parse().ok()?;
    Some(bytes / 1024)
}

pub fn capture_memory_cpu_snapshot() -> MemoryCpuSnapshot {
    let my_pid = std::process::id();
    let mut snap = MemoryCpuSnapshot {
        cgroup_memory_current_kb: read_cgroup_memory_current_kb(),
        ..Default::default()
    };

    if let Ok(entries) = fs::read_dir("/proc") {
        let mut procs: Vec<(u32, u32, String, u64)> = Vec::new();
        for entry in entries.flatten() {
            if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                let status_path = format!("/proc/{}/status", pid);
                if let Ok(status) = fs::read_to_string(status_path) {
                    let mut ppid = 0u32;
                    let mut rss = 0u64;
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
                        }
                    }
                    procs.push((pid, ppid, comm, rss));
                }
            }
        }

        let mut tree_pids = vec![my_pid];
        let mut changed = true;
        while changed {
            changed = false;
            for (pid, ppid, _, _) in &procs {
                if tree_pids.contains(ppid) && !tree_pids.contains(pid) {
                    tree_pids.push(*pid);
                    changed = true;
                }
            }
        }

        for (pid, _, comm, rss) in procs {
            if !tree_pids.contains(&pid) {
                continue;
            }
            let pss = read_pss_kb(pid);
            let cpu = read_cpu_ms(pid);
            snap.total_rss_kb += rss;
            snap.total_pss_kb += pss;
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
            target.cpu_ms += cpu;
        }
    }

    snap
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryPercentiles {
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

pub fn summarize(values: &[f64]) -> SummaryPercentiles {
    if values.is_empty() {
        return SummaryPercentiles {
            min: 0.0,
            median: 0.0,
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
    let p95_idx = ((n as f64) * 0.95).ceil() as usize;
    let p95 = s[p95_idx.saturating_sub(1).min(n - 1)];
    SummaryPercentiles {
        min: s[0],
        median,
        p95,
        max: s[n - 1],
    }
}
