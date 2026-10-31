// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-007 — host ResourceWatchSample on `sharecli proc` text/CSV surfaces
//! FR: FR-007
//!
//! AC-007.14 `proc` text and `--csv` emit host watch fields (parity with JSON host_watch)

use std::process::Command;

use sharecli::monitoring::HostResourceWatchJson;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

const TEXT_MARKERS: [&str; 5] = ["Open FDs:", "RSS:", "Load (1m):", "Net RX:", "Net TX:"];

const CSV_HEADER: &str = "record,fd_count,net_rx_bytes,net_tx_bytes,mem_rss_bytes,load_1m";

/// FR-007 / AC-007.14 — unit helper renders status-parity text section.
#[test]
fn fr007_host_watch_format_text_section() {
    let section = HostResourceWatchJson {
        fd_count: 42,
        net_rx_bytes: 1000,
        net_tx_bytes: 2000,
        mem_rss_bytes: 52_428_800,
        load_1m: 1.25,
    }
    .format_text_section();
    for marker in TEXT_MARKERS {
        assert!(section.contains(marker), "text section MUST include {marker}; got: {section}");
    }
    assert!(section.contains("42"), "text section MUST include fd_count; got: {section}");
    assert!(
        section.contains("52428800"),
        "text section MUST include mem_rss_bytes; got: {section}"
    );
}

/// FR-007 / AC-007.14 — unit helper renders companion CSV host record.
#[test]
fn fr007_host_watch_format_csv_companion() {
    let csv = HostResourceWatchJson {
        fd_count: 7,
        net_rx_bytes: 11,
        net_tx_bytes: 22,
        mem_rss_bytes: 4096,
        load_1m: 0.5,
    }
    .format_csv_companion();
    assert!(csv.contains(CSV_HEADER), "CSV companion MUST include host_watch header; got: {csv}");
    assert!(
        csv.contains("host,7,11,22,4096,0.50"),
        "CSV companion MUST include host data row; got: {csv}"
    );
}

/// FR-007 / AC-007.14 — CLI proc text surfaces live host resource watch footer.
#[test]
#[serial_test::serial]
fn fr007_proc_text_host_watch_footer() {
    let out = bin().args(["proc"]).output().expect("spawn sharecli proc");
    assert!(out.status.success(), "proc MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("=== Host Resource Watch ==="),
        "proc text MUST include host watch section (AC-007.14); got: {s}"
    );
    for marker in TEXT_MARKERS {
        assert!(s.contains(marker), "proc text MUST include {marker} (AC-007.14)");
    }
}

/// FR-007 / AC-007.14 — CLI proc --csv appends companion host_watch record block.
#[test]
#[serial_test::serial]
fn fr007_proc_csv_host_watch_companion() {
    let out = bin().args(["proc", "--csv"]).output().expect("spawn sharecli proc --csv");
    assert!(out.status.success(), "proc --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().any(|line| line == "pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"),
        "proc --csv MUST preserve agent header (AC-006.24); got: {s}"
    );
    assert!(
        s.contains(CSV_HEADER),
        "proc --csv MUST include host_watch CSV header (AC-007.14); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("host,")),
        "proc --csv MUST include host companion row (AC-007.14); got: {s}"
    );
}

/// FR-007 / AC-007.14 — CLI proc --tree text includes host watch footer.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_text_host_watch_footer() {
    let out = bin().args(["proc", "--tree"]).output().expect("spawn sharecli proc --tree");
    assert!(out.status.success(), "proc --tree MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("=== Host Resource Watch ==="),
        "proc --tree text MUST include host watch section (AC-007.14); got: {s}"
    );
}

/// FR-007 / AC-007.14 — CLI proc --tree --csv appends companion host_watch record.
#[test]
#[serial_test::serial]
fn fr007_proc_tree_csv_host_watch_companion() {
    let out =
        bin().args(["proc", "--tree", "--csv"]).output().expect("spawn sharecli proc --tree --csv");
    assert!(out.status.success(), "proc --tree --csv MUST exit 0; stderr: {:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.lines().next().unwrap_or("").starts_with("root_index,"),
        "proc --tree --csv MUST preserve tree header; got: {s}"
    );
    assert!(
        s.contains(CSV_HEADER),
        "proc --tree --csv MUST include host_watch CSV header (AC-007.14); got: {s}"
    );
    assert!(
        s.lines().any(|line| line.starts_with("host,")),
        "proc --tree --csv MUST include host companion row (AC-007.14); got: {s}"
    );
}
