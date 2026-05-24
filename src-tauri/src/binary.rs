use crate::{agent, paths};
use std::io;
use std::path::{Path, PathBuf};

/// Pure: given an npx root (e.g. ~/.npm/_npx), find the newest
/// `<hash>/node_modules/@screenpipe/cli-darwin-arm64/bin` dir that contains a `screenpipe` file.
pub fn find_npx_bin_dir(npx_root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(npx_root).ok()?;
    for entry in entries.flatten() {
        let bin = entry
            .path()
            .join("node_modules/@screenpipe/cli-darwin-arm64/bin");
        let sp = bin.join("screenpipe");
        if sp.is_file() {
            let mtime = std::fs::metadata(&sp)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            candidates.push((mtime, bin));
        }
    }
    candidates.sort_by_key(|(t, _)| *t);
    candidates.pop().map(|(_, p)| p)
}

/// Copy every file in `src_dir` into `dest_dir` (flat; the screenpipe bin dir is flat).
pub fn copy_dir_files(src_dir: &Path, dest_dir: &Path) -> io::Result<usize> {
    std::fs::create_dir_all(dest_dir)?;
    let mut n = 0;
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let dest = dest_dir.join(entry.file_name());
            std::fs::copy(entry.path(), &dest)?;
            n += 1;
        }
    }
    Ok(n)
}

fn npx_root() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".npm/_npx")
}

pub fn is_pinned() -> bool {
    paths::pinned_binary().is_file()
}

/// Populate the npx cache with the latest screenpipe, then return its bin dir.
fn ensure_npx_cached() -> io::Result<PathBuf> {
    // `--help` is enough to download+extract the package into the npx cache.
    let status = std::process::Command::new("npx")
        .args(["--yes", "screenpipe@latest", "record", "--help"])
        .output()?;
    if !status.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&status.stderr).into_owned(),
        ));
    }
    find_npx_bin_dir(&npx_root())
        .ok_or_else(|| io::Error::other("screenpipe bin dir not found in npx cache"))
}

/// First-time pin: copy current screenpipe (binary + metallib) into the pinned dir.
pub fn pin() -> io::Result<usize> {
    let src = ensure_npx_cached()?;
    copy_dir_files(&src, &paths::pinned_bin_dir())
}

/// Update: re-pin the latest, then restart the agent if loaded.
pub fn update() -> io::Result<usize> {
    let n = pin()?;
    if agent::is_installed() {
        agent::restart()?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_bin_dir_with_screenpipe() {
        let tmp = std::env::temp_dir().join(format!("spk-test-find-{}", std::process::id()));
        let bin = tmp.join("abc123/node_modules/@screenpipe/cli-darwin-arm64/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("screenpipe"), b"fake").unwrap();
        fs::write(bin.join("mlx.metallib"), b"fake").unwrap();

        let found = find_npx_bin_dir(&tmp).unwrap();
        assert_eq!(found, bin);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn copies_all_files_including_metallib() {
        let tmp = std::env::temp_dir().join(format!("spk-test-copy-{}", std::process::id()));
        let src = tmp.join("src");
        let dest = tmp.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("screenpipe"), b"bin").unwrap();
        fs::write(src.join("mlx.metallib"), b"lib").unwrap();

        let n = copy_dir_files(&src, &dest).unwrap();
        assert_eq!(n, 2);
        assert!(dest.join("screenpipe").is_file());
        assert!(dest.join("mlx.metallib").is_file());
        fs::remove_dir_all(&tmp).ok();
    }
}
