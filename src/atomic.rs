use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process,
};

use anyhow::{Context, Result};
use ulid::Ulid;

pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("yad");
    let temp = parent.join(format!(
        ".{}.tmp-{}-{}",
        file_name,
        process::id(),
        Ulid::new()
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .with_context(|| format!("failed to create temporary file {}", temp.display()))?;

        file.write_all(contents.as_ref())
            .with_context(|| format!("failed to write temporary file {}", temp.display()))?;
        file.flush()?;
        file.sync_all()
            .with_context(|| format!("failed to sync temporary file {}", temp.display()))?;
        drop(file);

        replace_file(&temp, path)?;

        #[cfg(unix)]
        {
            fs::File::open(parent)?.sync_all()?;
        }

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }

    result
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    unsafe extern "system" {
        fn MoveFileExW(
            lp_existing_file_name: *const u16,
            lp_new_file_name: *const u16,
            dw_flags: u32,
        ) -> i32;
    }

    let source_wide = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    let started = std::time::Instant::now();

    loop {
        let success = unsafe {
            MoveFileExW(
                source_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };

        if success != 0 {
            return Ok(());
        }

        let err = std::io::Error::last_os_error();
        let transient_file_contention = matches!(err.raw_os_error(), Some(5 | 32 | 33));

        if transient_file_contention && started.elapsed() < std::time::Duration::from_secs(2) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        return Err(err).with_context(|| {
            format!(
                "failed to atomically replace {} with {}",
                destination.display(),
                source.display()
            )
        });
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).with_context(|| {
        format!(
            "failed to atomically replace {} with {}",
            destination.display(),
            source.display()
        )
    })
}
