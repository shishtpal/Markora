//! Atomic local writes and content versions used for optimistic conflict detection.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum FileVersion {
    #[default]
    Missing,
    Present {
        bytes: u64,
        hash: u64,
    },
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    hash
}

impl FileVersion {
    pub fn of(bytes: &[u8]) -> Self {
        Self::Present {
            bytes: bytes.len() as u64,
            hash: hash_bytes(0xcbf29ce484222325, bytes),
        }
    }
    pub fn read(path: &Path) -> io::Result<Self> {
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::Missing),
            Err(e) => return Err(e),
        };
        let mut buffer = [0; 64 * 1024];
        let (mut bytes, mut hash) = (0, 0xcbf29ce484222325);
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            hash = hash_bytes(hash, &buffer[..count]);
        }
        Ok(Self::Present { bytes, hash })
    }
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn write_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_checked(path, contents, None)
}

pub fn write_checked(
    path: &Path,
    contents: &[u8],
    expected: Option<FileVersion>,
) -> io::Result<()> {
    write_with(path, expected, |file| file.write_all(contents))
}

fn check_version(path: &Path, expected: Option<FileVersion>) -> io::Result<()> {
    if let Some(expected) = expected
        && FileVersion::read(path)? != expected
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "File changed on disk",
        ));
    }
    Ok(())
}

fn write_with(
    path: &Path,
    expected: Option<FileVersion>,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    check_version(path, expected)?;
    let metadata = match fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_file() || meta.permissions().readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Destination is not a writable file",
                ));
            }
            Some(meta)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let (temp, mut file) = loop {
        let candidate = parent.join(format!(
            ".markora-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (Temporary(candidate), file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    if let Some(meta) = metadata {
        file.set_permissions(meta.permissions())?;
    }
    write(&mut file)?;
    file.sync_all()?;
    drop(file);
    check_version(path, expected)?;
    // Same-directory rename replaces the destination without first truncating it.
    fs::rename(&temp.0, path)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_write_and_racing_edit_preserve_destination_and_remove_temp() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/atomic-tests")
            .join(std::process::id().to_string());
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.md");
        fs::write(&path, "original").unwrap();
        let original = FileVersion::read(&path).unwrap();
        assert!(
            write_with(&path, Some(original), |file| {
                file.write_all(b"partial")?;
                Err(io::Error::other("simulated disk full"))
            })
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        assert!(
            write_with(&path, Some(original), |file| {
                fs::write(&path, "external")?;
                file.write_all(b"mine")
            })
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        write_checked(&path, b"complete", Some(FileVersion::of(b"external"))).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete");
        let new_path = dir.join("new.md");
        assert!(
            write_with(&new_path, Some(FileVersion::Missing), |file| {
                fs::write(&new_path, "created externally")?;
                file.write_all(b"mine")
            })
            .is_err()
        );
        assert_eq!(fs::read_to_string(new_path).unwrap(), "created externally");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn readonly_destination_is_not_replaced_or_made_writable() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/atomic-readonly")
            .join(std::process::id().to_string());
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("readonly.md");
        fs::write(&path, "protected").unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = write_file(&path, b"overwrite");
        let text = fs::read_to_string(&path).unwrap();
        let protected = fs::metadata(&path).unwrap().permissions().readonly();
        fs::set_permissions(&path, original).unwrap();
        fs::remove_dir_all(dir).unwrap();
        assert!(result.is_err());
        assert_eq!(text, "protected");
        assert!(protected);
    }
}
