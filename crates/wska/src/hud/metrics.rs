//! Machine metrics for the status line: pure parsers (ported from the weebo-hud mod) and the
//! file/process reads that feed them. Every source fails on its own and leaves its field empty.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mem {
    pub free: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    pub busy: f64,
    pub total: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    pub fs: String,
    pub avail: u64,
    pub size: u64,
    pub mount: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Metrics {
    pub ram: Option<Mem>,
    pub cpu: Option<f64>,
    pub disks: Vec<Disk>,
}

/// First unsigned integer after `key` (`key: 12` or `key 12`) at the start of a line.
fn num(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|l| {
        let rest = l.strip_prefix(key)?;
        let rest = rest.strip_prefix(':').unwrap_or(rest);
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        rest.split_whitespace().next()?.parse().ok()
    })
}

pub fn parse_meminfo(text: &str) -> Option<Mem> {
    let free = num(text, "MemAvailable")?.checked_mul(1024)?;
    let total = num(text, "MemTotal")?.checked_mul(1024)?;
    Some(Mem { free, total })
}

/// Kubelet's working set: page cache (inactive_file) is reclaimable, so it counts as free.
pub fn parse_cgroup_memory(max: &str, current: &str, stat: Option<&str>) -> Option<Mem> {
    let (max, current) = (max.trim(), current.trim());
    if max.is_empty() || current.is_empty() || max == "max" {
        return None;
    }
    let total: u64 = max.parse().ok()?;
    let cur: u64 = current.parse().ok()?;
    let inactive = stat.and_then(|s| num(s, "inactive_file")).unwrap_or(0);
    let used = cur.saturating_sub(inactive);
    Some(Mem {
        free: total.saturating_sub(used),
        total,
    })
}

pub fn parse_cpu_max(text: &str) -> Option<f64> {
    let mut it = text.split_whitespace();
    let quota = it.next()?;
    if quota == "max" {
        return None;
    }
    let cores = quota.parse::<f64>().ok()? / it.next()?.parse::<f64>().ok()?;
    cores.is_finite().then_some(cores)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "microsecond counters stay below 2^53"
)]
pub fn parse_cgroup_cpu_stat(text: &str) -> f64 {
    num(text, "usage_usec").unwrap_or(0) as f64
}

pub fn parse_proc_stat(text: &str) -> Option<Tick> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    // First 8 fields only: guest time is already counted inside user/nice.
    let f: Vec<f64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    let total: f64 = f.iter().sum();
    let at = |i: usize| f.get(i).copied().unwrap_or(0.0);
    Some(Tick {
        busy: total - at(3) - at(4),
        total,
    })
}

pub fn cpu_percent(prev: Tick, cur: Tick) -> Option<f64> {
    let dt = cur.total - prev.total;
    if dt <= 0.0 {
        return None;
    }
    let pct = (cur.busy - prev.busy) / dt * 100.0;
    pct.is_finite()
        .then(|| (pct.clamp(0.0, 100.0) * 10.0).round() / 10.0)
}

/// The header is localized, so skip line 1 whatever it says.
pub fn parse_df(text: &str) -> Vec<Disk> {
    let mut out: Vec<Disk> = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (Some(fs), Some(sz), Some(av), Some(mount)) = (f.first(), f.get(1), f.get(3), f.get(5))
        else {
            continue;
        };
        if out.iter().any(|d| d.fs == *fs) {
            continue;
        }
        let (Ok(size), Ok(avail)) = (sz.parse::<u64>(), av.parse::<u64>()) else {
            continue;
        };
        let (Some(size), Some(avail)) = (size.checked_mul(1024), avail.checked_mul(1024)) else {
            continue;
        };
        out.push(Disk {
            fs: (*fs).into(),
            avail,
            size,
            mount: (*mount).into(),
        });
    }
    out
}

pub fn format_state(source: &str, t: Tick, unix_ms: u128) -> String {
    format!("{source} {} {} {unix_ms}", t.busy, t.total)
}

/// The previous tick, only when it came from the same source (cgroup and proc units differ).
pub fn parse_state(text: &str, source: &str) -> Option<Tick> {
    let mut it = text.split_whitespace();
    if it.next()? != source {
        return None;
    }
    Some(Tick {
        busy: it.next()?.parse().ok()?,
        total: it.next()?.parse().ok()?,
    })
}

/// The unix-ms timestamp (4th field) of the stored sample.
pub fn parse_state_ms(text: &str) -> Option<u128> {
    text.split_whitespace().nth(3)?.parse().ok()
}

const CG: &str = "/sys/fs/cgroup/";

fn read(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn read_ram() -> Option<Mem> {
    let cg = match (
        read(&format!("{CG}memory.max")),
        read(&format!("{CG}memory.current")),
    ) {
        (Some(max), Some(cur)) => {
            parse_cgroup_memory(&max, &cur, read(&format!("{CG}memory.stat")).as_deref())
        }
        _ => None,
    };
    cg.or_else(|| parse_meminfo(&read("/proc/meminfo")?))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "wall-clock microseconds stay below 2^53"
)]
fn read_tick(now_ms: u128) -> Option<(&'static str, Tick)> {
    let cores = read(&format!("{CG}cpu.max")).and_then(|t| parse_cpu_max(&t));
    if let (Some(cores), Some(stat)) = (cores, read(&format!("{CG}cpu.stat"))) {
        // busy = usage_usec, total = wall-clock usec x quota cores (same unit).
        let total = (now_ms as f64) * 1000.0 * cores;
        return Some((
            "cgroup",
            Tick {
                busy: parse_cgroup_cpu_stat(&stat),
                total,
            },
        ));
    }
    Some(("proc", parse_proc_stat(&read("/proc/stat")?)?))
}

/// Sessions share the state file: a baseline younger than this is kept, so back-to-back runs don't see dt=0.
const MIN_BASELINE_AGE_MS: u128 = 1000;

/// Whether to replace the stored sample: when there is none, or it is old enough.
pub fn should_overwrite(stored_ms: Option<u128>, now_ms: u128) -> bool {
    stored_ms.is_none_or(|t| now_ms.saturating_sub(t) >= MIN_BASELINE_AGE_MS)
}

/// CPU share since the previous call; the state file is best-effort (a failure shows `—`).
fn read_cpu(state: &Path, now_ms: u128) -> Option<f64> {
    let (source, tick) = read_tick(now_ms)?;
    let text = fs::read_to_string(state).ok();
    let prev = text.as_deref().and_then(|t| parse_state(t, source));
    let stored_ms = text.as_deref().and_then(parse_state_ms);
    if should_overwrite(stored_ms, now_ms) {
        super::write_atomic(state, format_state(source, tick, now_ms).as_bytes()).ok()?;
    }
    cpu_percent(prev?, tick)
}

/// stdout of `cmd` (whatever its exit status), or None when it fails to start or outlives `limit`
/// (then killed, with one non-blocking reap attempt; init collects it if that misses).
/// shortcut: stdout is read after exit, so output past the pipe buffer (64 KiB) times out; fine for df.
fn run_with_deadline(mut cmd: Command, limit: Duration) -> Option<String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                child.kill().ok();
                // No blocking wait: a child stuck in D state would hang the status line; init reaps it at exit.
                child.try_wait().ok();
                return None;
            }
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    Some(out)
}

fn read_disks(paths: &[String]) -> Vec<Disk> {
    // Paths are separate arguments (never a shell string); LC_ALL=C because the header is localized.
    let mut cmd = Command::new("df");
    cmd.env("LC_ALL", "C").args(["-kP", "--"]).args(paths);
    run_with_deadline(cmd, Duration::from_secs(3))
        .map(|t| parse_df(&t))
        .unwrap_or_default()
}

pub fn sample(paths: &[String], state: &Path, now_ms: u128) -> Metrics {
    Metrics {
        ram: read_ram(),
        cpu: read_cpu(state, now_ms),
        disks: read_disks(paths),
    }
}

#[cfg(test)]
mod tests;
