use super::*;
use crate::hud::metrics::{Disk, Mem, Metrics};
use chrono::{TimeZone, Utc};
use serde_json::json;

fn texts(s: &[Segment]) -> Vec<String> {
    s.iter().map(|x| x.text.clone()).collect()
}
fn bgs(s: &[Segment]) -> Vec<u8> {
    s.iter().map(|x| x.bg).collect()
}
fn disk(mount: &str, avail: u64, size: u64) -> Disk {
    Disk {
        fs: mount.into(),
        avail,
        size,
        mount: mount.into(),
    }
}
const GIB: u64 = 1 << 30;

fn full_input() -> serde_json::Value {
    json!({
        "cwd": "/x/y/proj",
        "model": {"display_name": "Opus 5.5", "id": "claude-opus-5-5"},
        "context_window": {"used_percentage": 34.4},
        "rate_limits": {
            "five_hour": {"used_percentage": 62.0, "resets_at": 1_000_000_000},
            "seven_day": {"used_percentage": 21.0, "resets_at": 1_000_000_000}
        }
    })
}

#[test]
fn session_segments() {
    let s = build_segments(&full_input(), &Metrics::default(), &Options::default());
    let t = texts(&s);
    assert_eq!(t.first().map(String::as_str), Some("\u{f07b} proj"));
    assert_eq!(t.get(1).map(String::as_str), Some("★ Opus 5.5"));
    assert_eq!(t.get(2).map(String::as_str), Some("ctx 34%"));
    assert!(t.get(3).is_some_and(|x| x.starts_with("5h 62% \u{f021} ")));
    assert!(t.get(4).is_some_and(|x| x.starts_with("7j 21% \u{f021} ")));
    assert_eq!(&t[5..], ["RAM —", "CPU —", "disque —"]);
    assert_eq!(bgs(&s)[..5], [26, 99, 34, 178, 34]);
}

#[test]
fn workspace_dir_wins_and_model_falls_back_to_id() {
    let i =
        json!({"cwd": "/a/b", "workspace": {"current_dir": "/c/d"}, "model": {"id": "claude-x"}});
    let t = texts(&build_segments(
        &i,
        &Metrics::default(),
        &Options::default(),
    ));
    assert_eq!(t.first().map(String::as_str), Some("\u{f07b} d"));
    assert_eq!(t.get(1).map(String::as_str), Some("★ claude-x"));
}

#[test]
fn missing_data_is_a_dash() {
    let i = json!({"context_window": {"used_percentage": null}, "rate_limits": {"five_hour": {"used_percentage": 10}}});
    let t = texts(&build_segments(
        &i,
        &Metrics::default(),
        &Options::default(),
    ));
    assert_eq!(t[..5], ["\u{f07b} —", "★ —", "ctx —", "5h 10%", "7j —"]);
    // unusable stdin behaves as an empty object
    let e = build_segments(&json!({}), &Metrics::default(), &Options::default());
    assert_eq!(e.len(), 8);
}

#[test]
fn level_thresholds() {
    let ctx = |p: f64| {
        bgs(&build_segments(
            &json!({"context_window": {"used_percentage": p}}),
            &Metrics::default(),
            &Options::default(),
        ))[2]
    };
    assert_eq!(
        (ctx(49.0), ctx(50.0), ctx(69.0), ctx(70.0)),
        (34, 178, 178, 160)
    );
    let five = |p: f64| {
        bgs(&build_segments(
            &json!({"rate_limits": {"five_hour": {"used_percentage": p}}}),
            &Metrics::default(),
            &Options::default(),
        ))[3]
    };
    assert_eq!(
        (five(49.0), five(50.0), five(79.0), five(80.0)),
        (34, 178, 178, 160)
    );
    let cpu = |p: f64| {
        bgs(&build_segments(
            &json!({}),
            &Metrics {
                cpu: Some(p),
                ..Default::default()
            },
            &Options::default(),
        ))[6]
    };
    assert_eq!(
        (cpu(79.0), cpu(80.0), cpu(94.0), cpu(95.0)),
        (34, 178, 178, 160)
    );
    let ram = |f: u64| {
        bgs(&build_segments(
            &json!({}),
            &Metrics {
                ram: Some(Mem {
                    free: f,
                    total: 100,
                }),
                ..Default::default()
            },
            &Options::default(),
        ))[5]
    };
    assert_eq!((ram(20), ram(19), ram(10), ram(9)), (34, 178, 178, 160));
}

#[test]
fn machine_segments() {
    let m = Metrics {
        ram: Some(Mem {
            free: 24 * GIB,
            total: 32 * GIB,
        }),
        cpu: Some(11.6),
        disks: vec![disk("/", 120 * GIB, 500 * GIB)],
    };
    let t = texts(&build_segments(&json!({}), &m, &Options::default()));
    assert_eq!(
        t[5..],
        ["RAM 24.0/32.0G libre", "CPU 12%", "disque 120G libre"]
    );
}

#[test]
fn several_disks_are_labelled_by_mount_basename() {
    let m = Metrics {
        disks: vec![
            disk("/", 5 * GIB, 100 * GIB),
            disk("/var/projects", 50 * GIB, 100 * GIB),
        ],
        ..Default::default()
    };
    let t = texts(&build_segments(&json!({}), &m, &Options::default()));
    assert_eq!(
        t[7..],
        ["disque / 5.0G libre", "disque projects 50.0G libre"]
    );
}

#[test]
fn options_hide_segments_and_reset() {
    let o = Options::from_lookup(&|k| match k {
        "WEEBO_HUD_DOSSIER" | "WEEBO_HUD_RAM" => Some("OFF".into()),
        "WEEBO_HUD_RESET5H" => Some("0".into()),
        "WEEBO_HUD_RESET7J" => Some("no".into()),
        "WEEBO_HUD_CPU" => Some("yes".into()),
        _ => None,
    });
    let t = texts(&build_segments(&full_input(), &Metrics::default(), &o));
    assert_eq!(
        t,
        [
            "★ Opus 5.5",
            "ctx 34%",
            "5h 62%",
            "7j 21%",
            "CPU —",
            "disque —"
        ]
    );
}

#[test]
fn reset_format_local_and_french() {
    let d = Utc.with_ymd_and_hms(2026, 10, 24, 9, 5, 0).unwrap(); // a Saturday
    assert_eq!(format_at(&d, false), "09:05");
    assert_eq!(format_at(&d, true), "sam 09:05");
    let s = Utc.with_ymd_and_hms(2026, 10, 25, 23, 0, 0).unwrap();
    assert_eq!(format_at(&s, true), "dim 23:00");
}

#[test]
fn render_exact_ansi() {
    let segs = vec![
        Segment {
            text: "a".into(),
            bg: 26,
        },
        Segment {
            text: "b".into(),
            bg: 99,
        },
    ];
    assert_eq!(
        render(&segs),
        "\x1b[48;5;26m\x1b[38;5;255m a \x1b[48;5;99m\x1b[38;5;26m\u{e0b0}\x1b[38;5;255m b \x1b[0m\x1b[38;5;99m\u{e0b0}\x1b[0m"
    );
    assert_eq!(render(&[]), "");
}

#[test]
fn gib_format() {
    assert_eq!(gib(32 * GIB), "32.0");
    assert_eq!(gib(120 * GIB), "120");
}

#[test]
fn reset_accepts_float_epoch() {
    let mut i = full_input();
    i["rate_limits"]["five_hour"]["resets_at"] = json!(1_793_300_000.5);
    let t = texts(&build_segments(
        &i,
        &Metrics::default(),
        &Options::default(),
    ));
    assert!(t.get(3).is_some_and(|x| x.starts_with("5h 62% \u{f021} ")));
    assert_eq!(epoch_secs(&json!(1_793_300_000.5)), Some(1_793_300_000));
    assert_eq!(epoch_secs(&json!(1e300)), None);
    assert_eq!(epoch_secs(&json!("x")), None);
}
