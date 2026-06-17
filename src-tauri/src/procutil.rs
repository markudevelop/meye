use std::process::Command;

/// A `Command` that never flashes a console window on Windows. Every spawn of an
/// external tool (schtasks, tasklist, npx, the recorder CLI) from the GUI process
/// must go through this — a plain `Command::new` pops a visible conhost window for
/// each call on Windows. On other platforms this is a plain `Command`.
pub fn cmd<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}
