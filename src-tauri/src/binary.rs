use crate::{agent, paths, procutil};
use std::io;
use std::path::{Path, PathBuf};

/// Platform npm package dir (inside an npx cache entry) + upstream binary name.
#[cfg(target_os = "macos")]
pub const NPX_PKG_BIN: &str = "node_modules/@screenpipe/cli-darwin-arm64/bin";
#[cfg(windows)]
pub const NPX_PKG_BIN: &str = "node_modules/@screenpipe/cli-win32-x64/bin";
#[cfg(target_os = "macos")]
pub const UPSTREAM_BIN: &str = "screenpipe";
#[cfg(windows)]
pub const UPSTREAM_BIN: &str = "screenpipe.exe";

/// Pure: given an npx root (e.g. ~/.npm/_npx), find the newest
/// `<hash>/{NPX_PKG_BIN}` dir that contains the upstream screenpipe binary.
pub fn find_npx_bin_dir(npx_root: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(npx_root).ok()?;
    for entry in entries.flatten() {
        let bin = entry.path().join(NPX_PKG_BIN);
        let sp = bin.join(UPSTREAM_BIN);
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
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("npm-cache/_npx")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".npm/_npx")
    }
}

/// Ad-hoc signing identity now; swap to a Developer ID string later.
#[cfg(target_os = "macos")]
const SIGNING_IDENTITY: &str = "-";

/// Pure: the recorder bundle's Info.plist XML.
#[cfg(target_os = "macos")]
pub fn build_info_plist(bundle_id: &str, exec_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Meye Recorder</string>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleExecutable</key>
  <string>{exec_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Meye records audio so you can search and recall what you heard.</string>
  <key>NSScreenCaptureUsageDescription</key>
  <string>Meye records the screen so you can search and recall what you saw.</string>
  <key>NSAccessibilityUsageDescription</key>
  <string>Meye reads on-screen UI text so the timeline can capture what you were working on.</string>
  <key>NSLocalNetworkUsageDescription</key>
  <string>Meye serves your recordings to another Meye device on your local network when remote viewing is on.</string>
</dict>
</plist>
"#
    )
}

pub fn is_pinned() -> bool {
    paths::recorder_binary().is_file()
}

/// Self-heal: pre-rebrand installs have `screenpipe` in MacOS/ but the plist points at
/// `meye-recorder`. launchd then fails with EX_CONFIG and the service is stuck in
/// spawn-scheduled limbo — Start becomes a silent no-op. Rename + re-sign so the bundle
/// matches the plist.
///
/// Also self-heal Info.plist: older installs are missing NSScreenCaptureUsageDescription
/// and still declare CFBundleExecutable=screenpipe, both of which keep macOS re-prompting
/// for Screen Recording on every relaunch.
#[cfg(target_os = "macos")]
pub fn migrate_binary_name() {
    let mut changed = false;
    let upstream = paths::recorder_macos_dir().join(UPSTREAM_BIN);
    if !paths::recorder_binary().is_file() && upstream.is_file() {
        if std::fs::rename(&upstream, paths::recorder_binary()).is_ok() {
            changed = true;
        }
    }
    let needs_info = match std::fs::read_to_string(paths::recorder_info_plist()) {
        Ok(s) => {
            !s.contains("NSScreenCaptureUsageDescription")
                || !s.contains("NSLocalNetworkUsageDescription")
                || s.contains("<string>screenpipe</string>")
        }
        Err(_) => false,
    };
    if needs_info {
        if std::fs::write(
            paths::recorder_info_plist(),
            build_info_plist(paths::RECORDER_BUNDLE_ID, "meye-recorder"),
        )
        .is_ok()
        {
            changed = true;
        }
    }
    if changed {
        let _ = codesign(&paths::recorder_app());
    }
}

/// No rebrand legacy on Windows — installs were never made pre-rebrand here.
#[cfg(windows)]
pub fn migrate_binary_name() {}

/// Populate the npx cache with the latest screenpipe, then return its bin dir.
fn ensure_npx_cached() -> io::Result<PathBuf> {
    // On Windows npx is npx.cmd, which CreateProcess can't exec directly — go via cmd.
    #[cfg(windows)]
    let mut npx = {
        let mut c = procutil::cmd("cmd");
        c.args(["/c", "npx"]);
        c
    };
    #[cfg(not(windows))]
    let mut npx = procutil::cmd("npx");
    let status = npx
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

/// ad-hoc (or Developer ID) sign the recorder app bundle.
#[cfg(target_os = "macos")]
fn codesign(app: &Path) -> io::Result<()> {
    let out = std::process::Command::new("codesign")
        .args(["--force", "--deep", "--sign", SIGNING_IDENTITY])
        .arg(app)
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}

/// Pin screenpipe as a signed `.app` bundle so it gets a TCC identity (mic permission).
/// Copies the screenpipe binary + mlx.metallib into Contents/MacOS, writes Info.plist, signs.
#[cfg(target_os = "macos")]
pub fn pin() -> io::Result<usize> {
    let src = ensure_npx_cached()?;
    let macos = paths::recorder_macos_dir();
    let n = copy_dir_files(&src, &macos)?;
    // Rebrand: the upstream binary ships as `screenpipe`; rename to `meye-recorder`
    // so TCC, Activity Monitor, and `ps` all show our identity.
    let upstream = macos.join(UPSTREAM_BIN);
    if upstream.exists() {
        std::fs::rename(&upstream, paths::recorder_binary())?;
    }
    std::fs::write(
        paths::recorder_info_plist(),
        build_info_plist(paths::RECORDER_BUNDLE_ID, "meye-recorder"),
    )?;
    codesign(&paths::recorder_app())?;
    Ok(n)
}

/// Pin screenpipe into the recorder dir. No bundle/signing on Windows — just the exe
/// (+ any DLLs shipped beside it), rebranded so Task Manager / taskkill see our name.
#[cfg(windows)]
pub fn pin() -> io::Result<usize> {
    let src = ensure_npx_cached()?;
    // A running recorder holds a lock on its exe — copying over it fails. Best-effort
    // stop (also disables the keep-alive task so it doesn't relaunch mid-copy).
    if agent::is_running() {
        let _ = agent::stop();
    }
    let dest = paths::recorder_dir();
    let n = copy_dir_files(&src, &dest)?;
    let upstream = dest.join(UPSTREAM_BIN);
    if upstream.exists() {
        let _ = std::fs::remove_file(paths::recorder_binary()); // rename won't overwrite
        std::fs::rename(&upstream, paths::recorder_binary())?;
    }
    Ok(n)
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
        let bin = tmp.join("abc123").join(NPX_PKG_BIN);
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join(UPSTREAM_BIN), b"fake").unwrap();
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
        fs::write(src.join(UPSTREAM_BIN), b"bin").unwrap();
        fs::write(src.join("mlx.metallib"), b"lib").unwrap();

        let n = copy_dir_files(&src, &dest).unwrap();
        assert_eq!(n, 2);
        assert!(dest.join(UPSTREAM_BIN).is_file());
        assert!(dest.join("mlx.metallib").is_file());
        fs::remove_dir_all(&tmp).ok();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn info_plist_declares_identity_and_mic() {
        let xml = build_info_plist("com.meye.recorder", "meye-recorder");
        assert!(xml.contains("<key>CFBundleIdentifier</key>"));
        assert!(xml.contains("<string>com.meye.recorder</string>"));
        assert!(xml.contains("<key>CFBundleExecutable</key>"));
        assert!(xml.contains("<string>meye-recorder</string>"));
        assert!(xml.contains("<key>NSMicrophoneUsageDescription</key>"));
        assert!(xml.contains("<key>NSLocalNetworkUsageDescription</key>"));
        assert!(xml.contains("<key>LSUIElement</key>"));
    }
}
