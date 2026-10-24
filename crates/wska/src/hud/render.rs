//! Segments and their powerline ANSI rendering (palette and glyphs of the old statusline.sh).

use super::metrics::Metrics;
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use serde_json::Value;

const OK: u8 = 34;
const WARN: u8 = 178;
const CRIT: u8 = 160;
const SEP: char = '\u{e0b0}';
const DAYS: [&str; 7] = ["dim", "lun", "mar", "mer", "jeu", "ven", "sam"];

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: String,
    pub bg: u8,
}

/// One flag per segment (and per reset time); on unless the variable says otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub dossier: bool,
    pub modele: bool,
    pub contexte: bool,
    pub cinqh: bool,
    pub septj: bool,
    pub ram: bool,
    pub cpu: bool,
    pub disque: bool,
    /// Also sample the disk holding `$HOME`, not only the working directory's.
    pub disque_home: bool,
    pub reset5h: bool,
    pub reset7j: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self::from_lookup(&|_| None)
    }
}

impl Options {
    pub fn from_env() -> Self {
        Self::from_lookup(&|k| std::env::var(k).ok())
    }

    pub fn from_lookup(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let on = |name: &str| {
            let v = get(&format!("WEEBO_HUD_{name}"))
                .unwrap_or_default()
                .to_lowercase();
            !matches!(v.as_str(), "0" | "false" | "off" | "no")
        };
        Self {
            dossier: on("DOSSIER"),
            modele: on("MODELE"),
            contexte: on("CONTEXTE"),
            cinqh: on("CINQH"),
            septj: on("SEPTJ"),
            ram: on("RAM"),
            cpu: on("CPU"),
            disque: on("DISQUE"),
            disque_home: on("DISQUE_HOME"),
            reset5h: on("RESET5H"),
            reset7j: on("RESET7J"),
        }
    }
}

fn used(pct: f64, warn: f64, crit: f64) -> u8 {
    if pct >= crit {
        CRIT
    } else if pct >= warn {
        WARN
    } else {
        OK
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "byte counts only drive a percentage"
)]
fn free(free: u64, total: u64) -> u8 {
    let p = if total > 0 {
        free as f64 / total as f64 * 100.0
    } else {
        100.0
    };
    if p < 10.0 {
        CRIT
    } else if p < 20.0 {
        WARN
    } else {
        OK
    }
}

#[expect(clippy::cast_precision_loss, reason = "display only")]
pub fn gib(bytes: u64) -> String {
    let g = bytes as f64 / (1u64 << 30) as f64;
    if g >= 100.0 {
        format!("{g:.0}")
    } else {
        format!("{g:.1}")
    }
}

fn r0(x: f64) -> String {
    format!("{:.0}", (x + 0.5).floor())
}

pub fn format_at<Tz: TimeZone>(d: &DateTime<Tz>, with_day: bool) -> String {
    let hm = format!("{:02}:{:02}", d.hour(), d.minute());
    let day = DAYS
        .get(usize::try_from(d.weekday().num_days_from_sunday()).unwrap_or(0))
        .copied()
        .unwrap_or("");
    if with_day {
        format!("{day} {hm}")
    } else {
        hm
    }
}

/// Integer or float epoch seconds (the fraction is dropped); non-finite or absurd values are rejected.
fn epoch_secs(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    let f = v.as_f64()?.trunc();
    // 1e15 s is far past chrono's range and well inside i64, so the cast below cannot truncate.
    #[expect(clippy::cast_possible_truncation, reason = "range-checked just above")]
    (f.is_finite() && f.abs() < 1e15).then_some(f as i64)
}

fn format_reset(epoch: i64, with_day: bool) -> Option<String> {
    let d = DateTime::from_timestamp(epoch, 0)?.with_timezone(&Local);
    Some(format_at(&d, with_day))
}

fn basename(p: &str) -> Option<&str> {
    p.split('/').rfind(|s| !s.is_empty())
}

pub fn build_segments(input: &Value, m: &Metrics, o: &Options) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut add = |text: String, bg: u8| out.push(Segment { text, bg });
    let str_at = |p: &str| input.pointer(p).and_then(Value::as_str);

    if o.dossier {
        let dir = str_at("/workspace/current_dir").or_else(|| str_at("/cwd"));
        add(
            format!("\u{f07b} {}", dir.and_then(basename).unwrap_or("—")),
            26,
        );
    }
    if o.modele {
        let name = str_at("/model/display_name").or_else(|| str_at("/model/id"));
        add(format!("★ {}", name.unwrap_or("—")), 99);
    }
    if o.contexte {
        match input
            .pointer("/context_window/used_percentage")
            .and_then(Value::as_f64)
        {
            Some(p) => add(format!("ctx {}%", r0(p)), used(p, 50.0, 70.0)),
            None => add("ctx —".into(), OK),
        }
    }
    let mut limit = |label: &str, key: &str, reset: bool, with_day: bool| {
        let w = input.pointer(&format!("/rate_limits/{key}"));
        let Some(p) = w
            .and_then(|w| w.get("used_percentage"))
            .and_then(Value::as_f64)
        else {
            return add(format!("{label} —"), OK);
        };
        let at = w
            .and_then(|w| w.get("resets_at"))
            .and_then(epoch_secs)
            .filter(|_| reset);
        let when = at
            .and_then(|e| format_reset(e, with_day))
            .map(|t| format!(" \u{f021} {t}"));
        add(
            format!("{label} {}%{}", r0(p), when.unwrap_or_default()),
            used(p, 50.0, 80.0),
        );
    };
    if o.cinqh {
        limit("5h", "five_hour", o.reset5h, false);
    }
    if o.septj {
        limit("7j", "seven_day", o.reset7j, true);
    }
    if o.ram {
        match m.ram {
            Some(r) => add(
                format!("RAM {}/{}G libre", gib(r.free), gib(r.total)),
                free(r.free, r.total),
            ),
            None => add("RAM —".into(), OK),
        }
    }
    if o.cpu {
        match m.cpu {
            Some(c) => add(format!("CPU {}%", r0(c)), used(c, 80.0, 95.0)),
            None => add("CPU —".into(), OK),
        }
    }
    if o.disque {
        if m.disks.is_empty() {
            add("disque —".into(), OK);
        }
        // Several disks: name each by its mount's basename (`/` stays `/`); a single one stays unlabelled.
        let many = m.disks.len() > 1;
        for d in &m.disks {
            let label = if many {
                format!(" {}", basename(&d.mount).unwrap_or("/"))
            } else {
                String::new()
            };
            add(
                format!("disque{label} {}G libre", gib(d.avail)),
                free(d.avail, d.size),
            );
        }
    }
    out
}

pub fn render(segments: &[Segment]) -> String {
    let mut s = String::new();
    let mut prev: Option<u8> = None;
    for seg in segments {
        s.push_str(&format!("\x1b[48;5;{}m", seg.bg));
        if let Some(p) = prev {
            s.push_str(&format!("\x1b[38;5;{p}m{SEP}"));
        }
        s.push_str(&format!("\x1b[38;5;255m {} ", seg.text));
        prev = Some(seg.bg);
    }
    if let Some(p) = prev {
        s.push_str(&format!("\x1b[0m\x1b[38;5;{p}m{SEP}\x1b[0m"));
    }
    s
}

#[cfg(test)]
mod tests;
