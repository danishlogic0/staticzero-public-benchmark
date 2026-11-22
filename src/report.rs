use crate::runner::SingleRunSample;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Explicit declaration of the measured boundary and surfaces outside that boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeasurementScope {
    pub boundary_kind: &'static str,
    pub payload_inspection: &'static str,
    pub excluded_surfaces: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerIdentity {
    pub crate_version: &'static str,
    pub runner_binary_sha256: String,
    pub os: &'static str,
    pub arch: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AggregateMetrics {
    pub total_runs: usize,
    pub succeeded_runs: usize,
    pub failed_runs: usize,
    pub min_duration_micros: u64,
    pub median_duration_micros: u64,
    pub max_duration_micros: u64,
    pub max_observed_residual_files: u64,
    pub max_observed_residual_bytes_within_boundary: u64,
    pub any_persistence_surface_activity_observed: bool,
}

/// Scoped impact measurement report excluding workload payload content and raw argument strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactReport {
    pub report_format: &'static str,
    pub runner_identity: RunnerIdentity,
    pub target_program_basename: String,
    pub target_arg_count: usize,
    pub all_runs_succeeded: bool,
    pub scope: MeasurementScope,
    pub summary: AggregateMetrics,
    pub runs: Vec<SingleRunSample>,
}

impl ImpactReport {
    pub fn build(
        program: &str,
        arg_count: usize,
        runner_exe_path: &Path,
        samples: Vec<SingleRunSample>,
    ) -> io::Result<Self> {
        let binary_sha = compute_file_sha256(runner_exe_path)?;
        let basename = Path::new(program)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let total_runs = samples.len();
        let succeeded_runs = samples.iter().filter(|s| s.succeeded).count();
        let failed_runs = total_runs - succeeded_runs;

        let mut durations: Vec<u64> = samples.iter().map(|s| s.duration_micros).collect();
        durations.sort_unstable();

        let min_duration_micros = durations.first().copied().unwrap_or(0);
        let max_duration_micros = durations.last().copied().unwrap_or(0);
        let median_duration_micros = median_sorted(&durations);

        let max_observed_residual_files = samples
            .iter()
            .map(|s| s.residual.observed_files)
            .max()
            .unwrap_or(0);

        let max_observed_residual_bytes_within_boundary = samples
            .iter()
            .map(|s| s.residual.observed_bytes)
            .max()
            .unwrap_or(0);

        let any_activity = samples.iter().any(|s| {
            s.residual.observed_files > 0
                || s.residual.observed_directories > 0
                || s.residual.observed_bytes > 0
        });

        Ok(Self {
            report_format: "payload-blind-impact-report.prototype-0",
            runner_identity: RunnerIdentity {
                crate_version: env!("CARGO_PKG_VERSION"),
                runner_binary_sha256: binary_sha,
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
            },
            target_program_basename: basename,
            target_arg_count: arg_count,
            all_runs_succeeded: failed_runs == 0 && total_runs > 0,
            scope: MeasurementScope {
                boundary_kind: "isolated_per_run_working_directory",
                payload_inspection: "none_byte_counts_and_fs_metadata_only",
                excluded_surfaces: &[
                    "kernel_page_cache_and_ipc_buffers",
                    "process_memory_and_swap",
                    "filesystem_paths_outside_isolated_workdir",
                    "network_side_effects",
                ],
            },
            summary: AggregateMetrics {
                total_runs,
                succeeded_runs,
                failed_runs,
                min_duration_micros,
                median_duration_micros,
                max_duration_micros,
                max_observed_residual_files,
                max_observed_residual_bytes_within_boundary,
                any_persistence_surface_activity_observed: any_activity,
            },
            runs: samples,
        })
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

fn median_sorted(sorted: &[u64]) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2
    }
}

/// Computes the SHA-256 digest of the executing runner binary in fixed-size chunks.
pub fn compute_file_sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", byte);
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::ResidualFootprint;

    #[test]
    fn computes_known_sha256_vector() {
        let tmp = std::env::temp_dir().join("sha256-vector-test.bin");
        std::fs::write(&tmp, b"abc").unwrap();
        let digest = compute_file_sha256(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn calculates_median_for_odd_and_even_series() {
        assert_eq!(median_sorted(&[10, 40, 90]), 40);
        assert_eq!(median_sorted(&[10, 20, 40, 90]), 30);
    }

    #[test]
    fn builds_scoped_report_and_flags_residual_activity() {
        let exe = std::env::current_exe().unwrap();
        let samples = vec![
            SingleRunSample {
                iteration: 0,
                duration_micros: 120,
                exit_code: Some(0),
                succeeded: true,
                stdout_bytes: 16,
                stderr_bytes: 0,
                residual: ResidualFootprint {
                    observed_files: 0,
                    observed_directories: 0,
                    observed_bytes: 0,
                },
            },
            SingleRunSample {
                iteration: 1,
                duration_micros: 200,
                exit_code: Some(0),
                succeeded: true,
                stdout_bytes: 16,
                stderr_bytes: 0,
                residual: ResidualFootprint {
                    observed_files: 1,
                    observed_directories: 0,
                    observed_bytes: 42,
                },
            },
        ];

        let report = ImpactReport::build("/bin/sh", 2, &exe, samples).unwrap();
        assert_eq!(report.target_program_basename, "sh");
        assert_eq!(report.target_arg_count, 2);
        assert!(report.all_runs_succeeded);
        assert_eq!(report.summary.median_duration_micros, 160);
        assert_eq!(
            report.summary.max_observed_residual_bytes_within_boundary,
            42
        );
        assert!(report.summary.any_persistence_surface_activity_observed);
        assert_eq!(report.runner_identity.runner_binary_sha256.len(), 64);
    }
}
