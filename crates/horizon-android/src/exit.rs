//! The end of the process when the game ends it itself. On an error the game calls
//! `std::process::exit`, which is libc's `exit`: it runs the C++ destructors of the activity's
//! glue, and one of them deletes a JNI reference from this thread with another thread's
//! environment, which the Android runtime answers by aborting (seen on a phone: a plain
//! "resources are out of date" error ended as a crash with a thousand lines of thread dumps).
//!
//! The library is linked with `--wrap=exit` (build.rs): the game's exit comes here, says how it
//! ended in the file the launcher reads, lets the log reach its file, and leaves without the
//! destructors. Nothing of the process is used after it.
use std::ffi::c_int;
use std::path::PathBuf;
use std::sync::OnceLock;

static EXIT_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Where an exit is written down (`last-exit.txt` in the game's folder).
pub fn set_file(path: PathBuf) {
    let _ = EXIT_FILE.set(path);
}

/// # Safety
/// `exit`'s contract: none; it does not return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_exit(code: c_int) -> ! {
    eprintln!("Horizon for Android: the game left with code {code}");
    if let Some(file) = EXIT_FILE.get() {
        let _ = std::fs::write(file, format!("exit {code}\n"));
    }
    crate::output::flush();
    // SAFETY: `_exit` only ends the process.
    unsafe { libc::_exit(code) }
}
