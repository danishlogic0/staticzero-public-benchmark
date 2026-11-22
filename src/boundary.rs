use serde::Serialize;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Residual filesystem counts within the isolated working directory, collected via metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResidualFootprint {
    pub observed_files: u64,
    pub observed_directories: u64,
    pub observed_bytes: u64,
}

/// Per-run isolated working directory removed automatically on Drop.
pub struct IsolatedWorkdir {
    path: PathBuf,
}

impl IsolatedWorkdir {
    pub fn create(base_dir: &Path) -> io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir_name = format!("impact-run-{}-{}-{}", std::process::id(), nanos, seq);
        let path = base_dir.join(dir_name);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Counts residual entries and byte sizes using filesystem metadata without opening file contents.
    pub fn inspect_residual_surface(&self) -> io::Result<ResidualFootprint> {
        let mut footprint = ResidualFootprint {
            observed_files: 0,
            observed_directories: 0,
            observed_bytes: 0,
        };
        walk_metadata_only(&self.path, &mut footprint)?;
        Ok(footprint)
    }
}

impl Drop for IsolatedWorkdir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn walk_metadata_only(dir: &Path, out: &mut ResidualFootprint) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.is_dir() {
            out.observed_directories += 1;
            walk_metadata_only(&entry.path(), out)?;
        } else {
            out.observed_files += 1;
            out.observed_bytes += meta.len();
        }
    }
    Ok(())
}

/// Drains a byte stream (stdout/stderr) and returns only the byte count without retaining content.
pub fn drain_and_count<R: Read>(mut reader: R) -> io::Result<u64> {
    let mut total_bytes: u64 = 0;
    let mut scratch = [0u8; 4096];
    loop {
        let n = reader.read(&mut scratch)?;
        if n == 0 {
            break;
        }
        total_bytes += n as u64;
        scratch[..n].fill(0);
    }
    Ok(total_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn counts_residual_files_and_cleans_up_on_drop() {
        let base = std::env::temp_dir();
        let captured_path: PathBuf;
        {
            let workdir = IsolatedWorkdir::create(&base).expect("create workdir");
            captured_path = workdir.path().to_path_buf();

            let empty = workdir.inspect_residual_surface().expect("inspect empty");
            assert_eq!(empty.observed_files, 0);
            assert_eq!(empty.observed_bytes, 0);

            // Simulate a workload leaving 18 bytes inside a subdirectory.
            let sub = workdir.path().join("cache");
            fs::create_dir(&sub).unwrap();
            fs::write(sub.join("leftover.bin"), b"SECRET_PAYLOAD_123").unwrap();

            let after = workdir
                .inspect_residual_surface()
                .expect("inspect residual");
            assert_eq!(after.observed_directories, 1);
            assert_eq!(after.observed_files, 1);
            assert_eq!(after.observed_bytes, 18);
        }
        // Directory is removed automatically when workdir leaves scope.
        assert!(!captured_path.exists());
    }

    #[test]
    fn drains_stream_and_counts_bytes_only() {
        let stream = Cursor::new(b"SENSITIVE_CUSTOMER_OUTPUT".to_vec());
        let count = drain_and_count(stream).expect("drain");
        assert_eq!(count, 25);
    }
}
