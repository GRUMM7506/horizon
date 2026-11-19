//! What the Android activity and the game hand each other (docs/android-port.md): the activity's
//! `AndroidApp` for winit's event loop, the text its on-screen keyboard typed, its mouse, and the
//! game's requests to it. The library's entry point (`horizon-android`) fills this in from its JNI
//! calls, on whichever thread the system makes them; the game reads it on its own thread, once
//! per pass of its loop. Nothing here talks to the OS.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
pub use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();
static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static KEYBOARD: OnceLock<fn(bool)> = OnceLock::new();
static POINTER_CAPTURE: OnceLock<fn(bool)> = OnceLock::new();
static KEYBOARD_WANTED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_DISMISSED: AtomicBool = AtomicBool::new(false);
static POINTER_CAPTURED: AtomicBool = AtomicBool::new(false);

fn events() -> MutexGuard<'static, Vec<Event>> {
    EVENTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The activity the game runs in, before the event loop is made.
pub fn set_app(app: AndroidApp) {
    let _ = APP.set(app);
}

/// The activity the game runs in.
pub fn app() -> Option<AndroidApp> {
    APP.get().cloned()
}

/// Input the activity's own views deliver, which winit's Android backend doesn't.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Text the on-screen keyboard committed.
    Text(String),
    /// The on-screen keyboard's backspace.
    Backspace,
    /// The on-screen keyboard's action key.
    Enter,
    /// A mouse moved to this point of the surface, in pixels.
    MouseMoved([f32; 2]),
    /// A captured mouse moved by this much (camera look).
    MouseMotion([f32; 2]),
    /// A mouse button (0 left, 1 right, 2 middle) went down or up at this point.
    MouseButton {
        button: u8,
        down: bool,
        at: [f32; 2],
    },
    /// A mouse wheel turned, in lines: right and up positive.
    MouseWheel([f32; 2]),
}

/// Called by the activity's side, from any thread.
pub fn push(event: Event) {
    events().push(event);
}

/// Everything the activity delivered since the last call, in order.
pub fn take() -> Vec<Event> {
    std::mem::take(&mut *events())
}

/// How the game shows and hides the on-screen keyboard.
pub fn set_keyboard(show: fn(bool)) {
    let _ = KEYBOARD.set(show);
}

/// Show the on-screen keyboard while a text field has the focus, hide it otherwise. Asks the
/// activity only when the answer changes.
pub fn show_keyboard(show: bool) {
    if KEYBOARD_WANTED.swap(show, Ordering::Relaxed) != show {
        log::info!(
            "Horizon: Android keyboard {}",
            if show { "asked for" } else { "sent away" }
        );
        KEYBOARD_DISMISSED.store(false, Ordering::Relaxed);
        if let Some(keyboard) = KEYBOARD.get() {
            keyboard(show);
        }
    }
}

/// The player sent the keyboard away (the back gesture) while a text field keeps the focus.
pub fn keyboard_hidden() {
    KEYBOARD_DISMISSED.store(true, Ordering::Relaxed);
}

/// The player touched a screen whose text field still has the focus: a keyboard they sent
/// away comes back.
pub fn keyboard_back() {
    if KEYBOARD_WANTED.load(Ordering::Relaxed)
        && KEYBOARD_DISMISSED.swap(false, Ordering::Relaxed)
        && let Some(keyboard) = KEYBOARD.get()
    {
        keyboard(true);
    }
}

/// How the game captures a connected mouse for camera look.
pub fn set_pointer_capture(capture: fn(bool)) {
    let _ = POINTER_CAPTURE.set(capture);
}

/// Capture a connected mouse while the player looks around with it, release it over a screen.
pub fn capture_pointer(capture: bool) {
    if POINTER_CAPTURED.swap(capture, Ordering::Relaxed) != capture
        && {
            log::debug!("Horizon: Android pointer capture {capture}");
            true
        }
        && let Some(pointer) = POINTER_CAPTURE.get()
    {
        pointer(capture);
    }
}
