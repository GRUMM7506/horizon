//! The game as an Android library (docs/android-port.md). The launcher's `GameActivity`
//! (android/) loads `libhorizon_android.so`; android-activity's glue starts a native thread and
//! calls [`android_main`] on it, which runs the same game `horizon-game`'s executable does.
//!
//! The activity hands the game its folder and arguments through the environment before it loads
//! the library (`HORIZON_ANDROID_DIR`, `HORIZON_ANDROID_ARGS`), and its own views' input (the
//! on-screen keyboard's text, a mouse, controllers) through the JNI functions in [`bridge`].
#![cfg(target_os = "android")]

mod bridge;
mod output;
mod stack;

use std::path::PathBuf;
use winit::platform::android::activity::AndroidApp;

/// The file the launcher reads when the game activity is gone: how the last run ended.
const EXIT_FILE: &str = "last-exit.txt";

/// Called by android-activity on the activity's native thread.
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let dir = std::env::var_os("HORIZON_ANDROID_DIR")
        .map(PathBuf::from)
        .or_else(|| app.external_data_path().map(|path| path.join("horizon")))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    output::redirect(&dir);
    eprintln!(
        "Horizon for Android: folder {}, thread stack {} MiB, Android API {}",
        dir.display(),
        stack::current_size() / (1024 * 1024),
        AndroidApp::sdk_version()
    );
    // One argument per line, as the launcher wrote them.
    let args: Vec<String> = std::env::var("HORIZON_ANDROID_ARGS")
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    eprintln!("Horizon for Android: arguments {args:?}");
    // A panic's message and place, wherever it happens: the game's own crash report comes
    // later in its start, and only for the threads it knows.
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("Horizon for Android: {info}");
        earlier(info);
    }));
    bridge::install(&app);
    horizon_client_input::android::set_app(app.clone());
    let exit = dir.join(EXIT_FILE);
    let _ = std::fs::write(&exit, "running\n");
    let result = std::panic::catch_unwind(|| horizon_game::main_android(args, &dir, &dir));
    let status = match result {
        Ok(Ok(())) => "ok\n".to_owned(),
        Ok(Err(error)) => {
            eprintln!("Horizon: {error}");
            format!("error\n{error}\n")
        }
        Err(_) => "panic\n".to_owned(),
    };
    eprintln!("Horizon for Android: the game ended: {}", status.trim_end());
    let _ = std::fs::write(&exit, status);
    bridge::finish(&app);
    output::flush();
    // The game ran once in this process (its hooks and logs are set up for good): the activity's
    // process ends with it, and the launcher's own process shows again.
    // SAFETY: `_exit` only ends the process; nothing of it is used afterwards.
    unsafe { libc::_exit(0) }
}
