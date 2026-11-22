use crate::boundary::{drain_and_count, IsolatedWorkdir, ResidualFootprint};
use serde::Serialize;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

/// Parameters for a series of payload-blind measurement runs.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub program: String,
    pub args: Vec<String>,
    pub iterations: usize,
    pub base_temp_dir: PathBuf,
}

/// Measurement from a single isolated run, excluding stream and file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SingleRunSample {
    pub iteration: usize,
    pub duration_micros: u64,
    pub exit_code: Option<i32>,
    pub succeeded: bool,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub residual: ResidualFootprint,
}

#[derive(Debug)]
pub enum RunnerError {
    InvalidConfig(&'static str),
    Io(io::Error),
}

impl From<io::Error> for RunnerError {
    fn from(err: io::Error) -> Self {
        RunnerError::Io(err)
    }
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::InvalidConfig(msg) => write!(f, "invalid configuration: {}", msg),
            RunnerError::Io(err) => write!(f, "io error: {}", err),
        }
    }
}

impl std::error::Error for RunnerError {}

/// Executes the target command `iterations` times, each inside a fresh isolated working directory.
pub fn execute_series(config: &RunConfig) -> Result<Vec<SingleRunSample>, RunnerError> {
    if config.program.trim().is_empty() {
        return Err(RunnerError::InvalidConfig("program must not be empty"));
    }
    if config.iterations == 0 {
        return Err(RunnerError::InvalidConfig("iterations must be at least 1"));
    }

    let mut samples = Vec::with_capacity(config.iterations);
    for idx in 0..config.iterations {
        samples.push(execute_once(config, idx)?);
    }
    Ok(samples)
}

fn execute_once(config: &RunConfig, iteration: usize) -> Result<SingleRunSample, RunnerError> {
    let workdir = IsolatedWorkdir::create(&config.base_temp_dir)?;
    let work_path = workdir.path();

    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args)
        .current_dir(work_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", work_path)
        .env("TEMP", work_path)
        .env("TMP", work_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let started = Instant::now();
    let mut child = cmd.spawn()?;

    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing stdout pipe"))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing stderr pipe"))?;

    let (stdout_res, stderr_res, status_res) = thread::scope(|s| {
        let out_handle = s.spawn(|| drain_and_count(stdout_pipe));
        let err_handle = s.spawn(|| drain_and_count(stderr_pipe));
        let status = child.wait();
        let out_bytes = out_handle
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("stdout drain panicked")));
        let err_bytes = err_handle
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("stderr drain panicked")));
        (out_bytes, err_bytes, status)
    });

    let elapsed_micros = started.elapsed().as_micros() as u64;
    let status = status_res?;
    let stdout_bytes = stdout_res?;
    let stderr_bytes = stderr_res?;

    // Inspect residual directory surface before workdir is dropped and removed.
    let residual = workdir.inspect_residual_surface()?;

    Ok(SingleRunSample {
        iteration,
        duration_micros: elapsed_micros,
        exit_code: status.code(),
        succeeded: status.success(),
        stdout_bytes,
        stderr_bytes,
        residual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_command_leaves_zero_residual_bytes() {
        let cfg = RunConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "printf 'HELLO_PAYLOAD'".to_string()],
            iterations: 3,
            base_temp_dir: std::env::temp_dir(),
        };
        let samples = execute_series(&cfg).expect("run clean command");
        assert_eq!(samples.len(), 3);
        for s in &samples {
            assert!(s.succeeded);
            assert_eq!(s.exit_code, Some(0));
            assert_eq!(s.stdout_bytes, 13);
            assert_eq!(s.residual.observed_files, 0);
            assert_eq!(s.residual.observed_bytes, 0);
        }
    }

    #[test]
    fn detects_leftover_file_inside_boundary() {
        let cfg = RunConfig {
            program: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf '1234567890' > leaked.tmp".to_string(),
            ],
            iterations: 2,
            base_temp_dir: std::env::temp_dir(),
        };
        let samples = execute_series(&cfg).expect("run dirty command");
        for s in &samples {
            assert!(s.succeeded);
            assert_eq!(s.residual.observed_files, 1);
            assert_eq!(s.residual.observed_bytes, 10);
        }
    }

    #[test]
    fn records_non_zero_exit_code_fail_closed() {
        let cfg = RunConfig {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "exit 7".to_string()],
            iterations: 1,
            base_temp_dir: std::env::temp_dir(),
        };
        let samples = execute_series(&cfg).expect("run failing command");
        assert!(!samples[0].succeeded);
        assert_eq!(samples[0].exit_code, Some(7));
    }
}
