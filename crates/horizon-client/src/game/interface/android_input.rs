//! Android: the game's side of the activity it runs in (docs/android-port.md).
//!
//! Fingers: in the world they drive upstream's touch controls
//! (`horizon_client_render::touch`, the joystick, the buttons and the look gesture), which read
//! as a controller and reach the game through the controller's own path (`poll_pads`); over a
//! screen one finger is the mouse pointer (`touch_pointer`). The activity's own views deliver
//! what winit's Android backend doesn't: the on-screen keyboard's text, a mouse and controllers
//! (`horizon_client::android`, `android_runtime`). When the activity stops, the surface goes and
//! the sound stops; both come back with it.
//!
//! Nothing here changes what the game does with a key, a button or a stick: it only makes them.
#![cfg(target_os = "android")]
use super::*;
use horizon_client::android::{self, Event};
use horizon_client::gamepad::PadState;
use horizon_client::touch_pointer::{Pointer, TouchPointer};
use horizon_client_render::android_gamepad::InputMode;
use horizon_client_render::android_runtime;
use horizon_client_render::touch::{self, Control, TouchControls};
use std::cell::RefCell;
use winit::event::{Touch, TouchPhase};

/// The touch layout the player saved, in the game folder (`touch::Settings`).
const SETTINGS_FILE: &str = "horizon-touch.json";
/// A finger over a screen moves the pointer once it is this far from where it landed (a
/// fraction of the surface height).
const POINTER_SLOP: f32 = 0.02;
/// GUI pixels of a finger's swipe that scroll as one line of the mouse wheel.
const SCROLL_LINE: f32 = 12.0;
/// A world tap holds attack or use this long when the game isn't ticking (a tick otherwise).
const CLICK_TIME: Duration = Duration::from_millis(150);

struct Input {
    controls: TouchControls,
    pointer: TouchPointer,
    /// The attack or use a world tap pressed: its action, the tick and the time it went down.
    click: Option<(&'static str, u64, Instant)>,
    /// A tap on the world uses and a hold mines (the launcher's switch); else a tap attacks
    /// and a hold uses.
    tap_uses: bool,
    /// The controls were in use at the last poll.
    playing: bool,
}

impl Input {
    fn new() -> Self {
        let number = |name: &str| std::env::var(name).ok()?.parse::<f32>().ok();
        let mut settings = touch::Settings::load(std::path::Path::new(SETTINGS_FILE));
        // Attack and use act on the crosshair here: the schemes that act where a finger
        // lands need the game to aim there, which this glue does not do.
        if !settings.crosshair() {
            settings.mode = touch::Mode::Crosshair;
        }
        settings.crosshair_gestures = true;
        // No touch settings screen yet: the launcher holds them.
        settings.button_visibility[Control::Settings as usize] = touch::layout::HIDDEN;
        if let Some(scale) = number("HORIZON_TOUCH_SCALE") {
            settings.button_scale = scale;
        }
        if let Some(opacity) = number("HORIZON_TOUCH_OPACITY") {
            for placement in settings.layouts.iter_mut().flatten() {
                placement.opacity = opacity;
            }
        }
        settings.sanitize();
        let mut controls = TouchControls::default();
        controls.settings = settings;
        Self {
            controls,
            pointer: TouchPointer::default(),
            click: None,
            tap_uses: std::env::var("HORIZON_TOUCH_TAP_USES").is_ok_and(|v| v == "1"),
            playing: false,
        }
    }
}

thread_local! {
    /// The window thread's: one game draws on Android, and its server thread reads no input.
    static INPUT: RefCell<Input> = RefCell::new(Input::new());
}

/// The launcher's "detailed input log" switch (`HORIZON_ANDROID_LOG_INPUT=1`): every finger,
/// what it was taken for and what the game was told, in the game's log. For a report from a
/// phone ("this button does nothing"); off, nothing is formatted.
fn tracing() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var("HORIZON_ANDROID_LOG_INPUT").is_ok_and(|v| v == "1")
    });
    *ON
}

macro_rules! trace {
    ($($arg:tt)*) => {
        if tracing() {
            log::info!("Horizon input: {}", format_args!($($arg)*));
        }
    };
}

fn pointer_event(action: Pointer, device_id: DeviceId, scale: f32) -> WindowEvent {
    match action {
        Pointer::Moved([x, y]) => WindowEvent::CursorMoved {
            device_id,
            position: winit::dpi::PhysicalPosition::new(f64::from(x), f64::from(y)),
        },
        Pointer::Button(down) => WindowEvent::MouseInput {
            device_id,
            state: if down {
                ElementState::Pressed
            } else {
                ElementState::Released
            },
            button: MouseButton::Left,
        },
        Pointer::Scrolled(pixels) => WindowEvent::MouseWheel {
            device_id,
            delta: MouseScrollDelta::LineDelta(0.0, pixels / (scale.max(1.0) * SCROLL_LINE)),
            phase: TouchPhase::Moved,
        },
    }
}

/// A finger over a menu that has no game yet (the loading screen's title menu), as the mouse
/// events it stands for. `size` is the surface's and `scale` its GUI scale.
pub(super) fn menu_touch(touch: Touch, size: [u32; 2], scale: f32) -> Vec<WindowEvent> {
    let at = [touch.location.x as f32, touch.location.y as f32];
    let slop = size[1] as f32 * POINTER_SLOP;
    INPUT
        .with_borrow_mut(|input| {
            input
                .pointer
                .touch(touch.id, touch.phase, at, slop, Instant::now())
        })
        .into_iter()
        .map(|action| pointer_event(action, touch.device_id, scale))
        .collect()
}

/// Once per pass of the loop, for the same menu: a finger held still presses.
pub(super) fn menu_touch_poll(scale: f32) -> Vec<WindowEvent> {
    INPUT
        .with_borrow_mut(|input| input.pointer.poll(Instant::now()))
        .into_iter()
        .map(|action| pointer_event(action, DeviceId::dummy(), scale))
        .collect()
}

/// Android's Back (the button, the gesture, a keyboard's) is Escape: it closes the screen, or
/// pauses the game.
pub(super) fn back_is_escape(event: &mut WindowEvent) {
    if let WindowEvent::KeyboardInput { event: key, .. } = event
        && key.physical_key == PhysicalKey::Code(KeyCode::BrowserBack)
    {
        key.physical_key = PhysicalKey::Code(KeyCode::Escape);
        key.logical_key = winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape);
        key.text = None;
    }
}

/// The activity stopped: the window is going away, and the renderer lets go of its surface
/// before that. The device and everything loaded on it stay.
pub(super) fn window_lost(renderer: Option<&mut Renderer>) {
    INPUT.with_borrow_mut(|input| {
        input.controls.clear();
        input.pointer.cancel();
    });
    android_runtime::clear_gamepads();
    if let Some(renderer) = renderer {
        renderer.window_lost();
    }
    log::info!("Horizon: the activity stopped, the window's surface is released");
}

/// The activity is back with a new window surface, of whatever size it has now.
pub(super) fn window_back(renderer: Option<&mut Renderer>, window: &Window) -> Result<()> {
    let size = window.inner_size();
    if let Some(renderer) = renderer {
        renderer.window_restored(size.width, size.height)?;
    }
    // A screen that only draws when asked (the missing-resources one) is asked.
    window.request_redraw();
    log::info!(
        "Horizon: the activity is back, a new surface of {}x{}",
        size.width,
        size.height
    );
    Ok(())
}

impl Game<'_> {
    /// The player is in the world with no screen over it: fingers play, they don't point.
    fn android_playing(&self) -> bool {
        self.captured && self.screen == Screen::Playing && self.title.is_none()
    }

    fn android_surface(&self) -> Option<[u32; 2]> {
        self.renderer
            .as_ref()
            .map(|r| [r.config.width.max(1), r.config.height.max(1)])
    }

    /// A window event before the game's own handling: touches are used up here, Back becomes
    /// Escape, and everything else passes.
    pub(super) fn android_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        mut event: WindowEvent,
    ) -> Option<WindowEvent> {
        if let WindowEvent::Touch(touch) = event {
            self.android_touch(event_loop, id, touch);
            return None;
        }
        match &event {
            WindowEvent::Focused(focused) => log::info!("Horizon: Android window focus {focused}"),
            WindowEvent::Resized(size) => {
                log::info!("Horizon: Android window {}x{}", size.width, size.height);
            }
            WindowEvent::KeyboardInput { event: key, .. } => trace!(
                "key {:?} {:?} text {:?}",
                key.physical_key, key.state, key.text
            ),
            _ => {}
        }
        back_is_escape(&mut event);
        if let WindowEvent::KeyboardInput { event: key, .. } = &event
            && key.physical_key != PhysicalKey::Code(KeyCode::Escape)
            && matches!(key.physical_key, PhysicalKey::Code(_))
        {
            // A keyboard is in use: the touch controls step aside (their shortcuts stay).
            android_runtime::note_input(InputMode::KeyboardMouse);
        }
        Some(event)
    }

    fn android_touch(&mut self, event_loop: &ActiveEventLoop, id: WindowId, touch: Touch) {
        enum Step {
            Nothing,
            Turn([f32; 2]),
            Slot(u8),
            Pointer(Vec<Pointer>),
        }
        let Some(size) = self.android_surface() else {
            return;
        };
        let gui = HudBatch::new(size, self.settings.gui_scale_inputs());
        let at = [touch.location.x as f32, touch.location.y as f32];
        let point = [at[0] / size[0] as f32, at[1] / size[1] as f32];
        let playing = self.android_playing();
        let started = touch.phase == TouchPhase::Started;
        if started {
            android_runtime::note_input(InputMode::Touch);
            self.last_input = Instant::now();
        }
        let step = INPUT.with_borrow_mut(|input| {
            // A finger keeps what it landed on until it lifts, whatever opens meanwhile.
            let in_world = if started {
                playing
            } else {
                input.controls.owns_pointer(touch.id)
            };
            if !in_world {
                let slop = size[1] as f32 * POINTER_SLOP;
                return Step::Pointer(input.pointer.touch(
                    touch.id,
                    touch.phase,
                    at,
                    slop,
                    Instant::now(),
                ));
            }
            let controls = &mut input.controls;
            match touch.phase {
                TouchPhase::Started => {
                    controls.viewport(size[0], size[1]);
                    let slot = touch::hotbar_slot(
                        gui.width,
                        gui.height,
                        [at[0] / gui.scale, at[1] / gui.scale],
                    );
                    match slot {
                        // A tap on the hotbar selects its slot, unless a control lies over it.
                        Some(slot) if controls.control_at(point).is_none() => {
                            controls.ignore(touch.id);
                            Step::Slot(slot as u8)
                        }
                        _ => {
                            controls.update(touch.id, point, true, false);
                            Step::Nothing
                        }
                    }
                }
                TouchPhase::Moved => Step::Turn(controls.update(touch.id, point, false, false)),
                TouchPhase::Ended => {
                    controls.update(touch.id, point, false, true);
                    Step::Nothing
                }
                // The system took the finger (a gesture of its own): no tap, no hold.
                TouchPhase::Cancelled => {
                    controls.ignore(touch.id);
                    controls.update(touch.id, point, false, true);
                    Step::Nothing
                }
            }
        });
        if touch.phase != TouchPhase::Moved {
            trace!(
                "finger {} {:?} at {:.0},{:.0} ({}): {}",
                touch.id,
                touch.phase,
                at[0],
                at[1],
                if playing { "world" } else { "screen" },
                match &step {
                    Step::Nothing => "touch controls".to_owned(),
                    Step::Turn(_) => "camera".to_owned(),
                    Step::Slot(slot) => format!("hotbar slot {slot}"),
                    Step::Pointer(actions) => format!("pointer {actions:?}"),
                }
            );
        }
        match step {
            Step::Nothing => {}
            Step::Turn(delta) => {
                if delta != [0.0; 2] {
                    let [yaw, pitch] = INPUT.with_borrow(|input| {
                        touch::camera_delta(delta, size[0], size[1], &input.controls.settings)
                    });
                    // Degrees, as `Entity.turn` takes them (the control socket's `turn`).
                    self.turn_player(f64::from(yaw) / 0.15, f64::from(pitch) / 0.15);
                }
            }
            Step::Slot(slot) => {
                let _ = self.select_hotbar_slot(slot);
            }
            Step::Pointer(actions) => {
                if started && self.text_field_focused() {
                    // The player sent the keyboard away and touches the screen again.
                    android::keyboard_back();
                }
                for action in actions {
                    let event = pointer_event(action, touch.device_id, gui.scale);
                    self.handle_window_event(event_loop, id, event, input_trace::InputOrigin::User);
                }
            }
        }
    }

    /// Once per pass of the loop, before the controllers are read: the pointer finger's hold,
    /// and what the activity's views delivered since the last pass.
    pub(super) fn android_poll(&mut self, event_loop: &dyn EventLoopCtl) {
        let (Some(id), Some(size)) = (self.window.as_ref().map(|w| w.id()), self.android_surface())
        else {
            return;
        };
        let scale = HudBatch::new(size, self.settings.gui_scale_inputs()).scale;
        let playing = self.android_playing();
        let device_id = DeviceId::dummy();
        let actions = INPUT.with_borrow_mut(|input| {
            if playing {
                // The screen closed under the finger.
                input.pointer.cancel()
            } else {
                input.pointer.poll(Instant::now())
            }
        });
        let user = input_trace::InputOrigin::User;
        for action in actions {
            let event = pointer_event(action, device_id, scale);
            self.handle_window_event(event_loop, id, event, user);
        }
        for event in android::take() {
            if !matches!(event, Event::MouseMoved(_) | Event::MouseMotion(_)) {
                trace!(
                    "from the activity: {event:?} (text field focused: {})",
                    self.text_field_focused()
                );
            }
            let moved = |[x, y]: [f32; 2]| WindowEvent::CursorMoved {
                device_id,
                position: winit::dpi::PhysicalPosition::new(f64::from(x), f64::from(y)),
            };
            match event {
                Event::Text(_) | Event::Backspace | Event::Enter => {
                    if !self.text_field_focused() {
                        continue;
                    }
                    // The same as a key of the controller's on-screen keyboard: into the
                    // field that has the focus.
                    self.pad_osk_output(match event {
                        Event::Text(text) => horizon_client::osk::Output::Text(text),
                        Event::Backspace => horizon_client::osk::Output::Backspace,
                        _ => horizon_client::osk::Output::Enter,
                    });
                }
                Event::MouseMoved(at) => {
                    android_runtime::note_input(InputMode::KeyboardMouse);
                    if !self.captured {
                        self.handle_window_event(event_loop, id, moved(at), user);
                    }
                }
                Event::MouseButton { button, down, at } => {
                    android_runtime::note_input(InputMode::KeyboardMouse);
                    if !self.captured && at[0] >= 0.0 {
                        self.handle_window_event(event_loop, id, moved(at), user);
                    }
                    let event = WindowEvent::MouseInput {
                        device_id,
                        state: if down {
                            ElementState::Pressed
                        } else {
                            ElementState::Released
                        },
                        button: match button {
                            0 => MouseButton::Left,
                            1 => MouseButton::Right,
                            _ => MouseButton::Middle,
                        },
                    };
                    self.handle_window_event(event_loop, id, event, user);
                }
                Event::MouseWheel([x, y]) => {
                    let event = WindowEvent::MouseWheel {
                        device_id,
                        delta: MouseScrollDelta::LineDelta(x, y),
                        phase: TouchPhase::Moved,
                    };
                    self.handle_window_event(event_loop, id, event, user);
                }
                // A captured mouse: `MouseHandler.onMove` with the pointer grabbed, as the
                // window's `DeviceEvent::MouseMotion` elsewhere.
                Event::MouseMotion([x, y]) => {
                    if self.pending_save.is_none()
                        && self.captured
                        && self.focused
                        && self.screen == Screen::Playing
                    {
                        self.mouse
                            .on_move(0.0, 0.0, f64::from(x), f64::from(y), true, true);
                        self.last_input = Instant::now();
                    }
                }
            }
        }
    }

    /// The touch controls as a controller, once per poll of the controllers; and what they
    /// ask for besides (a tap on the world, the chat, F3, F5).
    pub(super) fn android_touch_pad(&mut self) -> PadState {
        let playing = self.android_playing();
        let size = self.android_surface().unwrap_or([1, 1]);
        let sneaking = self.keys.held(horizon_client::actions::Action::Sneak);
        let ticks = self.ticks;
        // A controller, a keyboard or a mouse plays: only the shortcut buttons stay.
        let other_device = android_runtime::active_input() != InputMode::Touch;
        let mut debug = false;
        let mut acts: Vec<(&'static str, bool)> = Vec::new();
        let state = INPUT.with_borrow_mut(|input| {
            if let Some((action, tick, since)) = input.click
                && (tick != ticks || since.elapsed() >= CLICK_TIME)
            {
                acts.push((action, false));
                input.click = None;
            }
            if !playing {
                if std::mem::take(&mut input.playing) {
                    input.controls.clear();
                }
                return PadState::default();
            }
            input.playing = true;
            let tap_uses = input.tap_uses;
            let controls = &mut input.controls;
            controls.viewport(size[0], size[1]);
            controls.sneaking = sneaking;
            controls.auxiliary_only = other_device;
            controls.hold_uses_item = !tap_uses;
            let state = controls.poll_state();
            if controls.take_tap().is_some() && input.click.is_none() {
                let action = if tap_uses { "key.use" } else { "key.attack" };
                acts.push((action, true));
                input.click = Some((action, ticks, Instant::now()));
            }
            for (token, down) in controls.key_events() {
                if token == "action:key.swapOffhand" {
                    acts.push(("key.swapOffhand", down));
                }
            }
            if std::mem::take(&mut controls.chat_requested) {
                acts.extend([("key.chat", true), ("key.chat", false)]);
            }
            if std::mem::take(&mut controls.perspective_requested) {
                acts.extend([
                    ("key.togglePerspective", true),
                    ("key.togglePerspective", false),
                ]);
            }
            debug = std::mem::take(&mut controls.debug_requested);
            controls.settings_requested = false;
            controls.clicked = false;
            state
        });
        if playing {
            // What the touch controls hold, when it changes.
            thread_local! {
                static LAST: std::cell::Cell<(u32, bool, [bool; 2])> = const { std::cell::Cell::new((0, false, [false; 2])) };
            }
            let now = (
                state.buttons,
                state.left != [0.0; 2],
                [state.triggers[0] > 0.0, state.triggers[1] > 0.0],
            );
            if tracing() && LAST.replace(now) != now {
                trace!(
                    "touch controls: buttons {:#x}, joystick {:?}, use {} attack {}",
                    state.buttons, state.left, now.2[0], now.2[1]
                );
            }
        }
        for (action, down) in acts {
            trace!("touch action {action} {}", if down { "down" } else { "up" });
            if let Err(e) = self.pad_action(action, down) {
                log::warn!("Horizon: touch control {action}: {e}");
            }
        }
        if debug {
            // F3 pressed and let go: the debug overlay toggles as the key comes up.
            use horizon_client::debug_keys::KeyAction;
            self.physical_keys.insert(KeyCode::F3);
            self.debug_key(KeyCode::F3, KeyAction::Press);
            self.physical_keys.remove(&KeyCode::F3);
            self.debug_key(KeyCode::F3, KeyAction::Release);
        }
        state
    }

    /// The touch controls over the HUD, while the player is in the world.
    pub(super) fn android_draw_touch(&self, batch: &mut HudBatch) {
        if self.android_playing() {
            INPUT.with_borrow(|input| input.controls.draw(batch, &self.gui.atlas));
        }
    }

    /// `ApplicationHandler::suspended`: the activity stopped.
    pub(super) fn android_suspended(&mut self) {
        log::info!(
            "Horizon: Android suspends the game (title screen: {}, in the world: {})",
            self.title.is_some(),
            self.android_playing()
        );
        window_lost(self.renderer.as_mut());
        self.audio.output_suspended();
    }

    /// `ApplicationHandler::resumed` with the window already made: the activity is back.
    pub(super) fn android_resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Err(e) = window_back(self.renderer.as_mut(), &window) {
            self.error = Some(e);
            event_loop.exit();
            return;
        }
        self.audio.output_resumed();
        // The screen may have turned meanwhile: lay everything out for the size it has now.
        self.handle_window_event(
            event_loop,
            window.id(),
            WindowEvent::Resized(window.inner_size()),
            input_trace::InputOrigin::Game,
        );
        self.previous = Instant::now();
    }
}
