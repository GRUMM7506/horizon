//! Where the game's standard output and error go on Android, which gives a process neither: to
//! logcat (tag `Horizon`) and to `logs/android-output.log` in the game folder, for a player
//! without a computer at hand. The game's own log (`logs/latest.log`) is written as elsewhere.
use std::ffi::{CString, c_char, c_int};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::Path;

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// `ANDROID_LOG_INFO`.
const INFO: c_int = 4;

fn logcat(line: &str) {
    if let Ok(text) = CString::new(line.replace('\0', " ")) {
        // SAFETY: both strings are NUL-terminated and live for the call.
        unsafe { __android_log_write(INFO, c"Horizon".as_ptr(), text.as_ptr()) };
    }
}

/// Points file descriptors 1 and 2 at a pipe, and starts the thread that reads it.
pub fn redirect(dir: &Path) {
    let mut ends = [0; 2];
    // SAFETY: `ends` holds two descriptors; dup2 only replaces 1 and 2 with the pipe's write end,
    // which is closed once they refer to it. The read end is owned by the reader below.
    let read = unsafe {
        if libc::pipe(ends.as_mut_ptr()) != 0 {
            return;
        }
        libc::dup2(ends[1], 1);
        libc::dup2(ends[1], 2);
        libc::close(ends[1]);
        OwnedFd::from_raw_fd(ends[0])
    };
    let logs = dir.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let mut file = std::fs::File::create(logs.join("android-output.log")).ok();
    let reader = std::thread::Builder::new().name("Android output".to_owned());
    let _ = reader.spawn(move || {
        for line in BufReader::new(std::fs::File::from(read))
            .split(b'\n')
            .map_while(Result::ok)
        {
            let line = String::from_utf8_lossy(&line);
            logcat(&line);
            if let Some(file) = &mut file {
                let _ = writeln!(file, "{line}");
            }
        }
    });
}

/// Lets the reader thread take what was written last, before the process ends.
pub fn flush() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    std::thread::sleep(std::time::Duration::from_millis(250));
}
