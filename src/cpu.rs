//! CPU utilization (/proc/stat) + system memory/load (/proc/meminfo, /proc/loadavg).

use std::fs;
use std::time::Instant;

#[derive(Default)]
pub struct CpuSampler {
    prev_total: u64,
    prev_idle: u64,
    prev_per: Vec<(u64, u64)>,
    pub cpu_percent: f64,
    pub per_core_percent: Vec<f64>,
    last: Option<Instant>,
}

fn parse_cpu_line(line: &str) -> Option<(u64, u64)> {
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() < 4 {
        return None;
    }
    let idle = nums[3] + nums.get(4).unwrap_or(&0);
    let mut total: u64 = nums.iter().sum();
    // guest/guest_nice are already counted in user/nice; avoid double count.
    if nums.len() > 8 {
        total -= nums[8];
    }
    Some((total, idle))
}

impl CpuSampler {
    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.last.is_none() {
            self.prime();
            self.last = Some(now);
            return;
        }
        if let Some(prev) = self.last {
            if now.duration_since(prev).as_millis() < 200 {
                return;
            }
        }
        self.prime();
        self.last = Some(now);
    }

    fn prime(&mut self) {
        let Ok(content) = fs::read_to_string("/proc/stat") else {
            return;
        };
        let mut lines = content.lines();
        if let Some(first) = lines.next() {
            if let Some((total, idle)) = parse_cpu_line(first) {
                let dt = total.saturating_sub(self.prev_total);
                let di = idle.saturating_sub(self.prev_idle);
                if dt > 0 {
                    self.cpu_percent = (1.0 - di as f64 / dt as f64) * 100.0;
                }
                self.prev_total = total;
                self.prev_idle = idle;
            }
        }
        let mut per: Vec<(f64, (u64, u64))> = Vec::new();
        for line in lines {
            if !line.starts_with("cpu") || line.starts_with("cpu ") {
                continue;
            }
            let Some((total, idle)) = parse_cpu_line(line) else {
                continue;
            };
            let idx = per.len();
            let prev = self.prev_per.get(idx).copied().unwrap_or((total, idle));
            let dt = total.saturating_sub(prev.0);
            let di = idle.saturating_sub(prev.1);
            let pct = if dt > 0 {
                (1.0 - di as f64 / dt as f64) * 100.0
            } else {
                0.0
            };
            per.push((pct, (total, idle)));
        }
        self.per_core_percent = per.iter().map(|(p, _)| *p).collect();
        self.prev_per = per.iter().map(|(_, t)| *t).collect();
    }
}

/// Read the CPU model name from /proc/cpuinfo (first "model name" line).
pub fn cpu_model() -> String {
    if let Ok(s) = fs::read_to_string("/proc/cpuinfo") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                if let Some(v) = rest.split(':').nth(1) {
                    return v.trim().to_string();
                }
            }
        }
    }
    "CPU".to_string()
}

pub struct SystemMem {
    pub mem_total_kb: u64,
    pub mem_avail_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
}

impl Default for SystemMem {
    fn default() -> Self {
        Self {
            mem_total_kb: 1,
            mem_avail_kb: 1,
            swap_total_kb: 0,
            swap_free_kb: 0,
            load1: 0.0,
            load5: 0.0,
            load15: 0.0,
        }
    }
}

impl SystemMem {
    pub fn tick(&mut self) {
        if let Ok(s) = fs::read_to_string("/proc/meminfo") {
            for line in s.lines() {
                let mut it = line.split_whitespace();
                let key = it.next().unwrap_or("");
                let val = it.next().and_then(|v| v.parse::<u64>().ok());
                let Some(v) = val else { continue };
                match key {
                    "MemTotal:" => self.mem_total_kb = v,
                    "MemAvailable:" => self.mem_avail_kb = v,
                    "SwapTotal:" => self.swap_total_kb = v,
                    "SwapFree:" => self.swap_free_kb = v,
                    _ => {}
                }
            }
        }
        if let Ok(s) = fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 3 {
                self.load1 = parts[0].parse().unwrap_or(0.0);
                self.load5 = parts[1].parse().unwrap_or(0.0);
                self.load15 = parts[2].parse().unwrap_or(0.0);
            }
        }
    }

    pub fn mem_used_pct(&self) -> f64 {
        let total = self.mem_total_kb.max(1);
        (1.0 - self.mem_avail_kb as f64 / total as f64) * 100.0
    }
    pub fn swap_used_pct(&self) -> f64 {
        if self.swap_total_kb == 0 {
            0.0
        } else {
            (1.0 - self.swap_free_kb as f64 / self.swap_total_kb as f64) * 100.0
        }
    }
    pub fn mem_used_gb(&self) -> f64 {
        (self.mem_total_kb.saturating_sub(self.mem_avail_kb)) as f64 / 1_048_576.0
    }
    pub fn mem_total_gb(&self) -> f64 {
        self.mem_total_kb as f64 / 1_048_576.0
    }
    pub fn swap_used_gb(&self) -> f64 {
        (self.swap_total_kb.saturating_sub(self.swap_free_kb)) as f64 / 1_048_576.0
    }
    pub fn swap_total_gb(&self) -> f64 {
        self.swap_total_kb as f64 / 1_048_576.0
    }
}
