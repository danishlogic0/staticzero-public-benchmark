use payload_blind_impact_runner::report::ImpactReport;
use payload_blind_impact_runner::runner::{execute_series, RunConfig};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

struct CliArgs {
    iterations: usize,
    output_path: Option<PathBuf>,
    program: String,
    program_args: Vec<String>,
}

fn parse_cli(raw: &[String]) -> Result<CliArgs, String> {
    let mut iterations: usize = 5;
    let mut output_path: Option<PathBuf> = None;
    let mut idx = 0;

    while idx < raw.len() {
        match raw[idx].as_str() {
            "--" => {
                idx += 1;
                break;
            }
            "--iterations" | "-n" => {
                idx += 1;
                let val = raw
                    .get(idx)
                    .ok_or_else(|| "missing value after --iterations".to_string())?;
                iterations = val
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --iterations value: {}", val))?;
                if iterations == 0 {
                    return Err("--iterations must be >= 1".to_string());
                }
                idx += 1;
            }
            "--output" | "-o" => {
                idx += 1;
                let val = raw
                    .get(idx)
                    .ok_or_else(|| "missing path after --output".to_string())?;
                output_path = Some(PathBuf::from(val));
                idx += 1;
            }
            "--help" | "-h" => {
                return Err(usage());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag: {}\n{}", other, usage()));
            }
            _ => break,
        }
    }

    let remaining = &raw[idx..];
    if remaining.is_empty() {
        return Err(format!("missing target command\n{}", usage()));
    }

    Ok(CliArgs {
        iterations,
        output_path,
        program: remaining[0].clone(),
        program_args: remaining[1..].to_vec(),
    })
}

fn usage() -> String {
    "Usage: payload-blind-impact-runner [--iterations N] [--output FILE] -- <command> [args...]"
        .to_string()
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let cli = match parse_cli(&raw_args) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{}", msg);
            return ExitCode::from(2);
        }
    };

    let exe_path = match env::current_exe() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("failed to resolve current executable: {}", err);
            return ExitCode::from(2);
        }
    };

    let config = RunConfig {
        program: cli.program.clone(),
        args: cli.program_args.clone(),
        iterations: cli.iterations,
        base_temp_dir: env::temp_dir(),
    };

    let samples = match execute_series(&config) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("execution error: {}", err);
            return ExitCode::from(2);
        }
    };

    let report = match ImpactReport::build(&cli.program, cli.program_args.len(), &exe_path, samples)
    {
        Ok(r) => r,
        Err(err) => {
            eprintln!("report generation error: {}", err);
            return ExitCode::from(2);
        }
    };

    let json = match report.to_pretty_json() {
        Ok(j) => j,
        Err(err) => {
            eprintln!("json serialization error: {}", err);
            return ExitCode::from(2);
        }
    };

    if let Some(path) = cli.output_path {
        if let Err(err) = fs::write(&path, format!("{}\n", json)) {
            eprintln!("failed to write output file: {}", err);
            return ExitCode::from(2);
        }
    } else {
        println!("{}", json);
    }

    if report.all_runs_succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
