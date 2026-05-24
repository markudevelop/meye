use std::path::PathBuf;

pub const LABEL: &str = "com.screenpipe.keeper";
pub const PORT: u16 = 3030;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME must be set"))
}

pub fn pinned_bin_dir() -> PathBuf {
    home().join("Library/Application Support/screenpipe-keeper/bin")
}

pub fn pinned_binary() -> PathBuf {
    pinned_bin_dir().join("screenpipe")
}

pub fn plist_path() -> PathBuf {
    home().join("Library/LaunchAgents/com.screenpipe.keeper.plist")
}

pub fn log_dir() -> PathBuf {
    home().join("Library/Logs/screenpipe-keeper")
}

pub fn out_log() -> PathBuf {
    log_dir().join("out.log")
}

pub fn err_log() -> PathBuf {
    log_dir().join("err.log")
}

pub fn data_dir() -> PathBuf {
    home().join(".screenpipe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_home() {
        let h = home();
        assert!(pinned_binary().starts_with(&h));
        assert!(plist_path().ends_with("com.screenpipe.keeper.plist"));
        assert_eq!(out_log().file_name().unwrap(), "out.log");
        assert!(data_dir().ends_with(".screenpipe"));
    }
}
