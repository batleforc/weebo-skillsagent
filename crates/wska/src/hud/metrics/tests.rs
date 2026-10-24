use super::*;

#[test]
fn meminfo_in_bytes() {
    assert_eq!(
        parse_meminfo("MemTotal: 32000000 kB\nMemFree: 1 kB\nMemAvailable: 24000000 kB\n"),
        Some(Mem {
            free: 24_000_000 * 1024,
            total: 32_000_000 * 1024
        })
    );
    assert_eq!(parse_meminfo("nothing"), None);
}

#[test]
fn cgroup_memory() {
    let m = |free, total| Some(Mem { free, total });
    assert_eq!(
        parse_cgroup_memory("4294967296\n", "1073741824\n", None),
        m(3_221_225_472, 4_294_967_296)
    );
    assert_eq!(parse_cgroup_memory("max\n", "123", None), None);
    assert_eq!(parse_cgroup_memory("", "100", None), None);
    assert_eq!(parse_cgroup_memory("100", "  ", None), None);
    assert_eq!(parse_cgroup_memory("abc", "1", None), None);
    let stat = "anon 1\ninactive_anon 2\nactive_file 3\ninactive_file 536870912\n";
    assert_eq!(
        parse_cgroup_memory("4294967296", "1073741824", Some(stat)),
        m(3_758_096_384, 4_294_967_296)
    );
    assert_eq!(
        parse_cgroup_memory("100", "50", Some("inactive_file 80\n")),
        m(100, 100)
    );
    assert_eq!(parse_cgroup_memory("100", "150", None), m(0, 100));
    assert_eq!(parse_cgroup_memory("100", "40", Some("")), m(60, 100));
    assert_eq!(
        parse_cgroup_memory("100", "40", Some("garbage")),
        m(60, 100)
    );
}

#[test]
fn cpu_max_and_stat() {
    assert_eq!(parse_cpu_max("200000 100000\n"), Some(2.0));
    assert_eq!(parse_cpu_max("max 100000\n"), None);
    assert_eq!(
        parse_cgroup_cpu_stat("usage_usec 5000000\nuser_usec 1\n"),
        5_000_000.0
    );
}

#[test]
fn proc_stat() {
    let t = parse_proc_stat("cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 1 1 1 1 1 0 0 0 0 0\n");
    assert_eq!(
        t,
        Some(Tick {
            busy: 200.0,
            total: 1000.0
        })
    );
    assert_eq!(parse_proc_stat("no cpu line here\n"), None);
}

#[test]
fn cpu_percent_is_busy_share() {
    let t = Tick {
        busy: 200.0,
        total: 1000.0,
    };
    assert_eq!(
        cpu_percent(
            t,
            Tick {
                busy: 300.0,
                total: 1200.0
            }
        ),
        Some(50.0)
    );
    assert_eq!(cpu_percent(t, t), None);
    assert_eq!(
        cpu_percent(
            t,
            Tick {
                busy: 900.0,
                total: 1100.0
            }
        ),
        Some(100.0)
    );
    let z = Tick {
        busy: 0.0,
        total: 0.0,
    };
    assert_eq!(
        cpu_percent(
            z,
            Tick {
                busy: f64::NAN,
                total: 10.0
            }
        ),
        None
    );
}

#[test]
fn df_skips_header_dedups_and_keeps_mount() {
    let text = [
        "Sys. de fichiers blocs de 1K Utilisé Disponible Capacité Monté sur",
        "/dev/nvme0n1p3 100 40 60 40% /",
        "/dev/nvme0n1p3 100 40 60 40% /var/home",
        "/dev/b 200 0 200 0% /projects",
    ]
    .join("\n");
    let d = parse_df(&text);
    assert_eq!(d.len(), 2);
    assert_eq!(
        d.first(),
        Some(&Disk {
            fs: "/dev/nvme0n1p3".into(),
            avail: 60 * 1024,
            size: 100 * 1024,
            mount: "/".into()
        })
    );
    assert_eq!(d.get(1).map(|x| x.mount.as_str()), Some("/projects"));
}

#[test]
fn state_roundtrip_and_source_mismatch() {
    let line = format_state(
        "proc",
        Tick {
            busy: 5.0,
            total: 10.0,
        },
        42,
    );
    assert_eq!(line, "proc 5 10 42");
    assert_eq!(
        parse_state(&line, "proc"),
        Some(Tick {
            busy: 5.0,
            total: 10.0
        })
    );
    assert_eq!(parse_state(&line, "cgroup"), None);
    assert_eq!(parse_state("junk", "proc"), None);
}

#[test]
fn deadline_kills_a_slow_child() {
    let start = std::time::Instant::now();
    let mut c = Command::new("sleep");
    c.arg("10");
    assert_eq!(run_with_deadline(c, Duration::from_millis(200)), None);
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn deadline_returns_fast_output() {
    let mut c = Command::new("echo");
    c.arg("hi");
    assert_eq!(
        run_with_deadline(c, Duration::from_secs(5)).as_deref(),
        Some("hi\n")
    );
}

#[test]
fn state_file_write_is_atomic() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("cpu");
    let _ = read_cpu(&p, 1000);
    let _ = read_cpu(&p, 2000);
    assert!(fs::read_dir(d.path())
        .unwrap()
        .all(|e| e.unwrap().file_name() == "cpu"));
}

#[test]
fn young_baseline_is_kept() {
    let line = format_state(
        "proc",
        Tick {
            busy: 1.0,
            total: 2.0,
        },
        5000,
    );
    assert_eq!(parse_state_ms(&line), Some(5000));
    assert_eq!(parse_state_ms("junk"), None);
    assert!(should_overwrite(None, 5000));
    assert!(!should_overwrite(Some(5000), 5999));
    assert!(should_overwrite(Some(5000), 6000));
    assert!(!should_overwrite(Some(9000), 4999)); // clock went back
}
