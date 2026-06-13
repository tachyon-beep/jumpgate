//! Confirm that `--chronicle` output contains per-station epilogue lines.
//! These are printer-only (PDR-0006: windows, never gates) and must be
//! present whenever the chronicle flag is set on a scenario with stations.
//!
//! `trophic_run` is an `[[example]]`, so `CARGO_BIN_EXE_*` is not set for it.
//! Invoke it through `cargo run --example` instead — cargo locates/builds the
//! example and forwards stdout.

use std::process::Command;

#[test]
fn per_station_epilogue_appears_in_chronicle() {
    let out = Command::new(env!("CARGO"))
        .args([
            "run",
            "-q",
            "-p",
            "jumpgate-core",
            "--example",
            "trophic_run",
            "--",
            "--scenario",
            "trophic",
            "--seed",
            "7",
            // One full diagnostics window (WINDOW_TICKS = 2000) so at least one
            // TrophicSample exists; the epilogue reads samples.last().
            "--ticks",
            "2000",
            "--chronicle",
        ])
        .output()
        .expect("cargo run --example trophic_run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Per-station epilogue block header must appear
    assert!(
        stdout.contains("=== per-station epilogue"),
        "no per-station epilogue in chronicle output:\n{stdout}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
