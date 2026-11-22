use serde_json::Value;
use std::process::Command;

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_payload-blind-impact-runner")
}

#[test]
fn cli_clean_execution_returns_zero_and_zero_residuals() {
    let out = Command::new(bin_path())
        .args(["--iterations", "3", "--", "/bin/sh", "-c", "printf 'ok'"])
        .output()
        .expect("run binary");

    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["all_runs_succeeded"], true);
    assert_eq!(v["summary"]["total_runs"], 3);
    assert_eq!(
        v["summary"]["max_observed_residual_bytes_within_boundary"],
        0
    );
    assert_eq!(
        v["summary"]["any_persistence_surface_activity_observed"],
        false
    );
}

#[test]
fn cli_detects_residual_file_activity_within_boundary() {
    let out = Command::new(bin_path())
        .args([
            "--iterations",
            "2",
            "--",
            "/bin/sh",
            "-c",
            "printf 'LEFTOVER24BYTES_________'> residual.dat",
        ])
        .output()
        .expect("run binary");

    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(
        v["summary"]["max_observed_residual_bytes_within_boundary"],
        24
    );
    assert_eq!(
        v["summary"]["any_persistence_surface_activity_observed"],
        true
    );
}

#[test]
fn cli_never_leaks_secret_payload_from_args_streams_or_files() {
    let secret = "TOP_SECRET_CUSTOMER_PAYLOAD_9942";
    let script = format!(
        "printf '{s}'; printf '{s}' >&2; printf '{s}' > leftover.txt",
        s = secret
    );

    let out = Command::new(bin_path())
        .args(["--iterations", "2", "--", "/bin/sh", "-c", &script])
        .output()
        .expect("run binary");

    assert_eq!(out.status.code(), Some(0));
    let raw_report = String::from_utf8(out.stdout).expect("utf8 json");

    // Verify payload-blindness: workload output and file content never appear in the report.
    assert!(
        !raw_report.contains(secret),
        "secret leaked into report: {}",
        raw_report
    );

    // Byte counters remain exact across stdout, stderr and residual file surface.
    let v: Value = serde_json::from_str(&raw_report).expect("valid json");
    assert_eq!(v["runs"][0]["stdout_bytes"], 32);
    assert_eq!(v["runs"][0]["stderr_bytes"], 32);
    assert_eq!(
        v["summary"]["max_observed_residual_bytes_within_boundary"],
        32
    );
}

#[test]
fn cli_returns_exit_code_one_when_target_fails() {
    let out = Command::new(bin_path())
        .args(["--iterations", "2", "--", "/bin/sh", "-c", "exit 9"])
        .output()
        .expect("run binary");

    assert_eq!(out.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["all_runs_succeeded"], false);
    assert_eq!(v["summary"]["failed_runs"], 2);
}
