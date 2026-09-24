#[cfg(windows)]
pub mod gui_error;
#[cfg(windows)]
pub mod lifecycle;
#[cfg(windows)]
pub mod overlay;
pub mod practice;
#[cfg(windows)]
pub mod practice_dialog;
pub mod scancode;
#[cfg(windows)]
pub mod settings;
pub mod settings_editor;
pub mod settings_help;
#[cfg(windows)]
pub mod tray;
pub mod winui_host;

use clickless_backend_api::{Button, Dir, NullOverlay, OutputBackend, OverlayBackend};
use clickless_config::Config;
use clickless_core::grid::OverlayFrame;
use clickless_core::{Action, KeyEvent, LogicalKey, MotionConfig, Outcome, Phase, StateMachine};
use std::collections::{HashMap, VecDeque};

/// Minimal modifier tracking for OS screenshot chords. While Win is held,
/// S belongs to Win+Shift+S and must reach the OS, never the engine.
#[derive(Debug, Default)]
struct ChordGuard {
    win_down: bool,
}

impl ChordGuard {
    const VK_LWIN: u32 = 0x5B;
    const VK_RWIN: u32 = 0x5C;
    const VK_S: u32 = 0x53;

    fn observe(&mut self, vk: u32, is_down: bool) {
        if vk == Self::VK_LWIN || vk == Self::VK_RWIN {
            self.win_down = is_down;
        }
    }

    fn blocks(&self, vk: u32) -> bool {
        self.win_down && vk == Self::VK_S
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredKey {
    vk: u32,
    is_down: bool,
    now_ms: u64,
}

pub struct WindowsHook<O: OutputBackend> {
    sm: StateMachine,
    out: O,
    overlay: Box<dyn OverlayBackend + Send>,
    shown_overlay: Option<OverlayFrame>,
    /// Frames the overlay must display; presented later by `flush_overlay`.
    overlay_queue: VecDeque<Option<OverlayFrame>>,
    deferred_keys: VecDeque<DeferredKey>,
    draining_deferred: bool,
    chord: ChordGuard,
    /// Monitor rects captured at startup so Apply can re-arm the grid.
    monitors: Vec<clickless_core::grid::Rect>,
    /// Builds an overlay for the applied theme. `None` keeps the current one.
    #[allow(clippy::type_complexity)]
    overlay_factory: Option<
        Box<
            dyn Fn(clickless_backend_api::overlay::OverlayTheme) -> Box<dyn OverlayBackend + Send>
                + Send,
        >,
    >,
    /// Last applied full config, so the Settings editor opens on live values.
    /// The engine keeps only leader/bindings/motion; theme lives here.
    applied_config: Option<Config>,
    settings_saved_paused: Option<bool>,
}

impl<O: OutputBackend> WindowsHook<O> {
    pub fn new(out: O) -> Self {
        Self {
            sm: StateMachine::new(),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
            overlay_queue: VecDeque::new(),
            deferred_keys: VecDeque::new(),
            draining_deferred: false,
            chord: ChordGuard::default(),
            monitors: Vec::new(),
            overlay_factory: None,
            applied_config: None,
            settings_saved_paused: None,
        }
    }

    pub fn with_config(
        out: O,
        leader: LogicalKey,
        bindings: HashMap<LogicalKey, Action>,
        motion: MotionConfig,
    ) -> Self {
        Self {
            sm: StateMachine::with_config(leader, bindings, motion),
            out,
            overlay: Box::new(NullOverlay),
            shown_overlay: None,
            overlay_queue: VecDeque::new(),
            deferred_keys: VecDeque::new(),
            draining_deferred: false,
            chord: ChordGuard::default(),
            monitors: Vec::new(),
            overlay_factory: None,
            applied_config: None,
            settings_saved_paused: None,
        }
    }

    /// Installs the grid overlay renderer. Defaults to a no-op renderer.
    pub fn set_overlay(&mut self, overlay: Box<dyn OverlayBackend + Send>) {
        self.overlay = overlay;
        self.shown_overlay = None;
        self.overlay_queue.clear();
        self.deferred_keys.clear();
        self.draining_deferred = false;
    }

    /// Processes one key and reports the per-event suppression decision.
    /// Cheap and side-effect free on the overlay: it only records the frame
    /// intent, never rasterizes or touches a window. Presentation happens in
    /// `flush_overlay`, called from the event loop.
    pub fn process_key(&mut self, vk: u32, is_down: bool, now_ms: u64) -> Result<Outcome, String> {
        self.chord.observe(vk, is_down);
        if self.chord.blocks(vk) {
            // Keys inside an OS chord (Win+Shift+S) belong to the screenshot
            // tool: pass them through without engine state changes.
            return Ok(Outcome::PASS);
        }
        let key = match scancode::vk_to_logical(vk) {
            Some(k) => k,
            None => return Ok(Outcome::PASS),
        };
        if !self.draining_deferred && self.must_present_subgrid() {
            self.defer_key(DeferredKey {
                vk,
                is_down,
                now_ms,
            })?;
            return Ok(Outcome {
                consumed: true,
                action: None,
            });
        }
        let phase = if is_down {
            Phase::Press
        } else {
            Phase::Release
        };
        let outcome = self.sm.on_event_outcome(KeyEvent::new(key, phase), now_ms);
        if let Some(a) = outcome.action {
            self.execute(a)?;
        }
        self.update_overlay_intent();
        Ok(outcome)
    }

    /// Records what should be on screen. Called from the keyboard callback.
    fn update_overlay_intent(&mut self) {
        if !self.draining_deferred && self.sm.layer() != clickless_core::Layer::Grid {
            // Capture left the grid (exit, pause, hide): queued frames and
            // deferred keys belong to the dead session and must not leak
            // into the next one. The hide itself still queues below and
            // presents in the loop, never in the callback.
            self.overlay_queue.clear();
            self.deferred_keys.clear();
            self.draining_deferred = false;
        }
        self.queue_overlay_intent(self.sm.grid_overlay());
    }

    fn queue_overlay_intent(&mut self, frame: Option<OverlayFrame>) {
        if self.overlay_queue.back() == Some(&frame) {
            return;
        }
        if self.overlay_queue.is_empty() && self.shown_overlay == frame {
            return;
        }
        self.overlay_queue.push_back(frame);
    }

    fn must_present_subgrid(&self) -> bool {
        self.overlay_queue
            .iter()
            .any(|frame| frame.as_ref().is_some_and(is_required_subgrid_frame))
    }

    fn defer_key(&mut self, key: DeferredKey) -> Result<(), String> {
        const MAX_DEFERRED_KEYS: usize = 16;
        if self.deferred_keys.len() >= MAX_DEFERRED_KEYS {
            self.release_capture();
            return Err("deferred input queue overflow while presenting subgrid".to_string());
        }
        self.deferred_keys.push_back(key);
        Ok(())
    }

    /// Captures the monitor rects used by Apply to re-arm the grid.
    pub fn set_monitors(&mut self, monitors: Vec<clickless_core::grid::Rect>) {
        self.monitors = monitors;
    }

    /// Supplies a factory so Apply can rebuild the overlay for a new theme.
    pub fn set_overlay_factory(
        &mut self,
        factory: Box<
            dyn Fn(clickless_backend_api::overlay::OverlayTheme) -> Box<dyn OverlayBackend + Send>
                + Send,
        >,
    ) {
        self.overlay_factory = Some(factory);
    }

    /// The last applied config for editor seeding: the stored full config if
    /// one was applied at runtime, otherwise the config the hook booted with.
    pub fn with_boot_config(mut self, config: Config) -> Self {
        self.applied_config = Some(config);
        self
    }

    pub fn current_config(&self) -> Config {
        self.applied_config.clone().unwrap_or_default()
    }

    /// Applies a validated configuration at a safe boundary: exits any
    /// active layer, swaps leader/bindings/motion in core, re-arms the grid
    /// from the captured monitors and rebuilds the themed overlay.
    pub fn apply_config(&mut self, config: Config) -> Result<(), String> {
        self.release_capture();
        self.applied_config = Some(config.clone());
        let motion = clickless_core::MotionConfig {
            start_speed_px_s: config.settings.start_speed_px_s,
            max_speed_px_s: config.settings.max_speed_px_s,
            ramp_ms: config.settings.ramp_ms,
        };
        self.sm.reconfigure(
            config.settings.leader,
            config.mouse_bindings.clone(),
            motion,
        );
        self.sm.set_hold_ms(config.settings.hold_ms);
        let grid = config.grid.clone();
        if !self.monitors.is_empty() {
            self.sm
                .enable_grid_with_monitors(self.monitors.clone(), (0, 0), grid);
        }
        if let Some(factory) = self.overlay_factory.take() {
            self.overlay = factory(config.theme.to_overlay_theme());
            self.overlay_factory = Some(factory);
            self.shown_overlay = None;
            self.overlay_queue.clear();
            self.deferred_keys.clear();
            self.draining_deferred = false;
        }
        if self.settings_saved_paused.is_some() {
            // Settings owns focus: capture stays suspended; the applied
            // enabled state becomes the intent focus-leave restores.
            self.settings_saved_paused = Some(!config.enabled);
        } else if let Some(action) = self.sm.set_paused(!config.enabled) {
            self.execute(action)?;
        }
        Ok(())
    }

    /// Presents the queued overlay frame. Called from the event loop between
    /// message drains, never from the keyboard callback.
    pub fn flush_overlay(&mut self) -> Result<(), String> {
        let Some(next) = self.overlay_queue.pop_front() else {
            return Ok(());
        };
        if next == self.shown_overlay {
            return Ok(());
        }
        let presented_subgrid = next.as_ref().is_some_and(is_required_subgrid_frame);
        match next.clone() {
            Some(frame) => {
                self.overlay.show(&frame)?;
                self.shown_overlay = Some(frame);
            }
            None => {
                self.overlay.hide()?;
                self.shown_overlay = None;
            }
        }
        if presented_subgrid {
            self.drain_deferred_keys()?;
        }
        Ok(())
    }

    pub fn hide_overlay(&mut self) -> Result<(), String> {
        self.overlay_queue.clear();
        self.deferred_keys.clear();
        self.draining_deferred = false;
        if self.shown_overlay.take().is_some() {
            self.overlay.hide()?;
        }
        Ok(())
    }

    pub fn tick(&mut self, dt_ms: u64) -> Result<(), String> {
        for (dx, dy) in self.sm.tick(dt_ms) {
            self.out.move_rel(dx as i32, dy as i32)?;
        }
        Ok(())
    }

    pub(crate) fn execute(&mut self, action: Action) -> Result<(), String> {
        let step = self
            .applied_config
            .as_ref()
            .map(|config| config.settings.scroll_step)
            .unwrap_or(1)
            .max(1) as i32;
        match action {
            Action::ClickLeft => self.out.click(Button::Left)?,
            Action::ClickRight => self.out.click(Button::Right)?,
            Action::ScrollUp => self.out.scroll(0, step)?,
            Action::ScrollDown => self.out.scroll(0, -step)?,
            Action::MoveTo(x, y) => self.out.move_abs(x as i32, y as i32)?,
            Action::ClickAt(x, y) => {
                self.out.move_abs(x as i32, y as i32)?;
                self.out.click(Button::Left)?;
            }
            Action::DragTo(x, y) => {
                self.out.move_abs(x as i32, y as i32)?;
                self.out.button(Button::Left, Dir::Down)?;
            }
            Action::DragEnd => self.out.button(Button::Left, Dir::Up)?,
            _ => {}
        }
        Ok(())
    }

    pub fn sm(&self) -> &StateMachine {
        &self.sm
    }

    pub fn sm_mut(&mut self) -> &mut StateMachine {
        &mut self.sm
    }

    pub fn is_intercepting(&self) -> bool {
        self.sm.layer() == clickless_core::Layer::Mouse
            || self.sm.layer() == clickless_core::Layer::Grid
    }

    /// Tray Enable/pause. Pausing forces an exit, which ends any held drag
    /// before capture stops, and hides the overlay. Loop-side caller, so it
    /// presents immediately.
    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> {
        if self.settings_saved_paused.is_some() {
            // Settings owns focus: capture stays suspended, but the tray
            // intent is recorded so focus-leave restores it, not stale state.
            self.settings_saved_paused = Some(paused);
            return Ok(());
        }
        self.apply_paused(paused)
    }

    fn apply_paused(&mut self, paused: bool) -> Result<(), String> {
        if let Some(action) = self.sm.set_paused(paused) {
            self.execute(action)?;
        }
        self.update_overlay_intent();
        self.flush_overlay()
    }

    /// Settings-focus suspension. While the Settings window owns focus,
    /// capture is paused so ordinary typing edits controls. When focus
    /// leaves, restore the exact paused/enabled state that existed before.
    pub fn suspend_for_settings_focus(&mut self, focused: bool) -> Result<(), String> {
        match (focused, self.settings_saved_paused) {
            (true, None) => {
                let was_paused = self.sm.is_paused();
                self.settings_saved_paused = Some(was_paused);
                self.apply_paused(true)
            }
            (false, Some(was_paused)) => {
                self.settings_saved_paused = None;
                self.apply_paused(was_paused)
            }
            _ => Ok(()),
        }
    }

    /// Tray Show/hide grid. Show activates the grid overlay without a leader
    /// hold; hide forces an exit back to the initial layer, releasing any
    /// app-held drag button. Loop-side caller, so it presents immediately.
    pub fn show_grid(&mut self, show: bool) -> Result<(), String> {
        if show {
            self.sm.show_grid();
        } else if let Some(action) = self.sm.force_exit() {
            self.execute(action)?;
        }
        self.update_overlay_intent();
        self.flush_overlay()
    }

    /// Escalation stop used on errors and shutdown: full reset to the
    /// initial layer, releasing any app-held button, overlay hidden.
    /// Best effort; secondary failures must not mask the primary error.
    pub fn release_capture(&mut self) {
        if let Some(action) = self.sm_mut().force_exit() {
            let _ = self.execute(action);
        }
        let _ = self.hide_overlay();
    }

    pub fn out(&self) -> &O {
        &self.out
    }

    fn drain_deferred_keys(&mut self) -> Result<(), String> {
        self.draining_deferred = true;
        while let Some(event) = self.deferred_keys.pop_front() {
            if let Err(err) = self.process_key(event.vk, event.is_down, event.now_ms) {
                self.draining_deferred = false;
                return Err(err);
            }
        }
        self.draining_deferred = false;
        Ok(())
    }
}

fn is_required_subgrid_frame(frame: &OverlayFrame) -> bool {
    frame.level == 2
        && frame
            .cells
            .iter()
            .filter(|cell| cell.label.chars().count() == 1)
            .count()
            == 30
        && frame
            .cells
            .iter()
            .filter(|cell| cell.label.chars().count() == 2)
            .count()
            == 299
}

/// Desktop runtime loop: keyboard hook, tick pacing, tray command handling and
/// the settings-request poll. `tray` is `None` for headless/CI runs.
/// Windows-only: Linux and macOS crates carry their own event loops.
#[cfg(windows)]
pub fn run_event_loop<O: OutputBackend + Send + 'static>(
    hook: WindowsHook<O>,
    mut tray: Option<crate::tray::TrayMenu>,
    mut is_running: impl FnMut() -> bool,
    mut on_settings_request: impl FnMut() -> bool,
    practice_window: Option<crate::practice_dialog::PracticeWindow>,
) -> Result<(), String> {
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_REMOVE, PeekMessageW,
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static HOOK_PTR: AtomicPtr<WindowsHookState> = AtomicPtr::new(null_mut());

    struct WindowsHookState {
        hook: WindowsHook<Box<dyn OutputBackend + Send>>,
        start_time: Instant,
        settings_window: Option<SettingsHost>,
        practice_window: Option<crate::practice_dialog::PracticeWindow>,
        error: Option<String>,
        quit_requested: bool,
    }

    impl WindowsHookState {
        fn on_open_settings(&mut self, seed: Config) {
            if self
                .settings_window
                .as_ref()
                .is_some_and(|window| !window.is_visible())
            {
                self.settings_window = None;
            }
            if self.settings_window.is_none() {
                self.settings_window = open_settings_window(seed);
                if self.settings_window.is_none() {
                    crate::gui_error::log_event("settings open failed");
                    return; // retried on the next request
                }
            }
            if let Some(window) = self.settings_window.as_ref() {
                window.show();
            }
        }

        /// Records the first failure, releases capture so the user keeps
        /// control, and stops the loop. Secondary cleanup errors are best
        /// effort; the primary error is what reaches the CLI.
        fn fail(&mut self, err: String) {
            crate::gui_error::log_event(&format!("runtime stopped: {err}"));
            if self.error.is_none() {
                self.error = Some(err);
            }
            self.release_and_hide();
            self.quit_requested = true;
        }

        /// Best-effort cleanup shared by Quit and error shutdown.
        fn release_and_hide(&mut self) {
            self.hook.release_capture();
        }
    }

    unsafe extern "system" fn low_level_keyboard_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 {
            let state_ptr = HOOK_PTR.load(Ordering::SeqCst);
            if !state_ptr.is_null() {
                let state = unsafe { &mut *state_ptr };
                if state
                    .settings_window
                    .as_ref()
                    .is_some_and(|window| window.has_focus())
                {
                    return unsafe { CallNextHookEx(null_mut(), n_code, w_param, l_param) };
                }
                let kbd = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
                let is_down = w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN;
                let is_up = w_param as u32 == WM_KEYUP || w_param as u32 == WM_SYSKEYUP;
                if (is_down || is_up) && state.error.is_none() {
                    let now_ms = state.start_time.elapsed().as_millis() as u64;
                    match state.hook.process_key(kbd.vkCode, is_down, now_ms) {
                        // Suppress only what the engine consumed; every other
                        // key reaches the focused application unchanged.
                        Ok(outcome) if outcome.consumed => return 1,
                        Ok(_) => {}
                        Err(e) => state.fail(e),
                    }
                }
            }
        }
        unsafe { CallNextHookEx(null_mut(), n_code, w_param, l_param) }
    }

    /// Settings surface the loop drives. The default build hosts the native
    /// Win32 shell; the `winui3` feature swaps in the WinUI 3 shell.
    #[allow(dead_code)]
    enum SettingsHost {
        Native(crate::settings::win::SettingsWindow),
        #[cfg(feature = "winui3")]
        WinUi(crate::winui_host::enabled::WinUiSettings),
    }

    impl SettingsHost {
        fn show(&self) {
            match self {
                Self::Native(window) => window.show(),
                #[cfg(feature = "winui3")]
                Self::WinUi(window) => {
                    if let Err(reason) = window.show() {
                        crate::gui_error::log_event(&format!("settings raise failed: {reason}"));
                    }
                }
            }
        }

        /// True while the window owns the foreground; the hook passes keys
        /// through instead of consuming them then.
        fn has_focus(&self) -> bool {
            match self {
                Self::Native(window) => window.has_focus(),
                #[cfg(feature = "winui3")]
                Self::WinUi(window) => window.has_focus(),
            }
        }

        fn is_visible(&self) -> bool {
            match self {
                Self::Native(window) => window.is_visible(),
                #[cfg(feature = "winui3")]
                Self::WinUi(window) => window.is_visible(),
            }
        }

        /// Dialog navigation for the native shell. The WinUI shell handles its
        /// own control navigation, so its messages are only dispatched.
        fn translate_message(&self, msg: &MSG) -> bool {
            match self {
                Self::Native(window) => window.translate_message(msg),
                #[cfg(feature = "winui3")]
                Self::WinUi(_) => false,
            }
        }
    }

    /// Pushes an accepted config into the running hook. Apply runs on the loop
    /// thread inside a window message, when no `&mut state` borrow is live, so
    /// it reaches the hook through `HOOK_PTR` like the keyboard callback does.
    fn runtime_apply(config: &Config) -> Result<(), String> {
        let state_ptr = HOOK_PTR.load(Ordering::SeqCst);
        if state_ptr.is_null() {
            return Err("runtime is shutting down".to_string());
        }
        // SAFETY: valid on this thread for the loop's lifetime; no active
        // borrow while a window proc runs.
        let state = unsafe { &mut *state_ptr };
        state.hook.apply_config(config.clone())
    }

    /// Opens the settings shell on the running config. WinUI 3 when the feature
    /// is built in and its runtime is available, the native Win32 shell
    /// otherwise, so a missing Windows App SDK never leaves the owner without
    /// Settings. A failure is retried on the next request.
    fn open_settings_window(seed: Config) -> Option<SettingsHost> {
        #[cfg(feature = "winui3")]
        match crate::winui_host::enabled::WinUiSettings::create(
            seed.clone(),
            Box::new(|config: &Config| runtime_apply(config)),
        ) {
            Ok(window) => {
                window.notice();
                return Some(SettingsHost::WinUi(window));
            }
            Err(reason) => crate::gui_error::log_event(&format!(
                "WinUI settings unavailable ({reason}); using native settings host"
            )),
        }
        crate::settings::seed_settings(seed);
        match crate::settings::win::SettingsWindow::with_on_apply(Box::new(|config: &Config| {
            runtime_apply(config)
        })) {
            Ok(window) => Some(SettingsHost::Native(window)),
            Err(error) => {
                eprintln!("settings window unavailable: {error}");
                None
            }
        }
    }

    let out_boxed: Box<dyn OutputBackend + Send> = Box::new(hook.out);
    let mut state = WindowsHookState {
        hook: WindowsHook {
            sm: hook.sm,
            out: out_boxed,
            overlay: hook.overlay,
            shown_overlay: None,
            overlay_queue: VecDeque::new(),
            deferred_keys: VecDeque::new(),
            draining_deferred: false,
            chord: ChordGuard::default(),
            monitors: hook.monitors,
            overlay_factory: hook.overlay_factory,
            applied_config: hook.applied_config,
            settings_saved_paused: hook.settings_saved_paused,
        },
        start_time: Instant::now(),
        settings_window: None,
        practice_window,
        error: None,
        quit_requested: false,
    };
    HOOK_PTR.store(&mut state as *mut _, Ordering::SeqCst);

    let h_hook: HHOOK = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            null_mut() as HINSTANCE,
            0,
        )
    };

    if h_hook.is_null() {
        HOOK_PTR.store(null_mut(), Ordering::SeqCst);
        return Err("Failed to install WH_KEYBOARD_LL hook".to_string());
    }

    let mut last_tick = Instant::now();
    let mut msg: MSG = unsafe { std::mem::zeroed() };

    while is_running() && !state.quit_requested {
        unsafe {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if state
                    .practice_window
                    .as_ref()
                    .is_some_and(|window| window.translate_message(&msg))
                {
                    continue;
                }
                if state
                    .settings_window
                    .as_ref()
                    .is_some_and(|window| window.translate_message(&msg))
                {
                    continue;
                }
                DispatchMessageW(&msg);
            }
        }
        // A second launch signalled the named event: open Settings here, on
        // the thread that owns the windows.
        if on_settings_request() {
            state.on_open_settings(state.hook.current_config());
        }
        // The Settings "Practice again" button raises practice the same way.
        if crate::practice_dialog::PRACTICE_OPEN_REQUEST
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            if let Some(window) = state.practice_window.as_ref() {
                window.show();
                window.reset_state();
            } else {
                match crate::practice_dialog::PracticeWindow::new() {
                    Ok(window) => {
                        window.show();
                        state.practice_window = Some(window);
                    }
                    Err(e) => eprintln!("practice window unavailable: {e}"),
                }
            }
        }
        let settings_focused = state
            .settings_window
            .as_ref()
            .is_some_and(|window| window.has_focus());
        let practice_focused = state
            .practice_window
            .as_ref()
            .is_some_and(|window| window.has_focus());
        if let Err(e) = state
            .hook
            .suspend_for_settings_focus(settings_focused || practice_focused)
        {
            state.fail(e);
        }

        // Tray menu commands from the muda event channel. Failures release
        // capture and stop the loop instead of unwinding past the unhook.
        if let Some(tray) = tray.as_mut() {
            while let Ok(event) = crate::tray::menu_events().try_recv() {
                let command = tray.resolve(&event.id);
                match command {
                    Some(crate::tray::MenuCommand::ShowGrid) => {
                        if let Err(e) = state.hook.show_grid(true) {
                            state.fail(e);
                        }
                    }
                    Some(crate::tray::MenuCommand::HideGrid) => {
                        if let Err(e) = state.hook.show_grid(false) {
                            state.fail(e);
                        }
                    }
                    Some(crate::tray::MenuCommand::TogglePause) => {
                        let paused = !state.hook.sm().is_paused();
                        match state.hook.set_paused(paused) {
                            Ok(()) => tray.set_paused(paused),
                            Err(e) => state.fail(e),
                        }
                    }
                    Some(crate::tray::MenuCommand::OpenSettings) => {
                        crate::gui_error::log_event("tray settings command");
                        state.on_open_settings(state.hook.current_config());
                    }
                    Some(crate::tray::MenuCommand::OpenPractice) => {
                        crate::practice_dialog::PRACTICE_OPEN_REQUEST
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    Some(crate::tray::MenuCommand::Quit) => {
                        state.release_and_hide();
                        state.quit_requested = true;
                    }
                    None => {}
                }
            }
        }

        if state.quit_requested {
            break;
        }
        // Present queued overlay frames between message drains; the keyboard
        // callback only records the intent.
        if let Err(e) = state.hook.flush_overlay() {
            state.fail(e);
        }
        if state.error.is_none() {
            let now = Instant::now();
            let dt_ms = now.duration_since(last_tick).as_millis() as u64;
            if dt_ms >= 10 {
                if let Err(e) = state.hook.tick(dt_ms) {
                    state.fail(e);
                }
                last_tick = now;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    state.release_and_hide();
    unsafe {
        UnhookWindowsHookEx(h_hook);
    }
    HOOK_PTR.store(null_mut(), Ordering::SeqCst);
    match state.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_backend_api::{Button, Dir, OutputBackend};

    struct MockOut {
        moves: Vec<(i32, i32)>,
        abs: Vec<(i32, i32)>,
        buttons: Vec<(Button, Dir)>,
        scrolls: Vec<(i32, i32)>,
    }

    impl MockOut {
        fn new() -> Self {
            Self {
                moves: Vec::new(),
                abs: Vec::new(),
                buttons: Vec::new(),
                scrolls: Vec::new(),
            }
        }
    }

    impl OutputBackend for MockOut {
        fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.moves.push((dx, dy));
            Ok(())
        }

        fn move_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.abs.push((x, y));
            Ok(())
        }
        fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
            self.buttons.push((b, d));
            Ok(())
        }
        fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.scrolls.push((dx, dy));
            Ok(())
        }
    }

    fn enter_mouse(hook: &mut WindowsHook<MockOut>) {
        let _ = hook.process_key(0x14, true, 0); // CapsLock press
        let _ = hook.process_key(0x14, true, 200); // poll past threshold
    }

    // tray lifecycle commands

    #[test]
    fn t20_set_paused_ends_drag_and_hides_overlay() {
        use clickless_core::grid::GridConfig;
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                drag_after_select: true,
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // grid
        hook.process_key(0x4B, true, 400).unwrap();
        hook.process_key(0x4B, true, 500).unwrap();
        hook.process_key(0x4B, false, 600).unwrap(); // drag started

        hook.set_paused(true).unwrap();
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
        assert!(hook.sm().is_paused());
        assert!(hook.sm().grid_overlay().is_none());

        hook.set_paused(false).unwrap();
        assert!(!hook.sm().is_paused());
        assert_eq!(hook.process_key(0x14, true, 700).unwrap().action, None);
        assert_eq!(hook.process_key(0x14, true, 900).unwrap().action, None);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t21_show_grid_shows_overlay_without_leader() {
        use clickless_core::grid::GridConfig;
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());

        hook.show_grid(true).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Grid);

        hook.show_grid(false).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t01_unmapped_vk_yields_none() {
        let mut hook = WindowsHook::new(MockOut::new());
        assert_eq!(hook.process_key(0x0D, true, 0).unwrap(), Outcome::PASS); // VK_RETURN
    }

    #[test]
    fn t02_capslock_hold_enters_mouse_layer() {
        let mut hook = WindowsHook::new(MockOut::new());
        hook.process_key(0x14, true, 0).unwrap();
        hook.process_key(0x14, true, 200).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
    }

    #[test]
    fn t03_j_in_mouse_yields_move_right() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4A, true, 300).unwrap().action,
            Some(Action::MoveRight)
        );
    }

    #[test]
    fn t04_h_in_mouse_yields_move_left() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x48, true, 300).unwrap().action,
            Some(Action::MoveLeft)
        );
    }

    #[test]
    fn t05_k_in_mouse_yields_move_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4B, true, 300).unwrap().action,
            Some(Action::MoveUp)
        );
    }

    #[test]
    fn t06_l_in_mouse_yields_move_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4C, true, 300).unwrap().action,
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn t07_f_in_mouse_yields_click_left() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x46, true, 300).unwrap().action,
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t08_d_in_mouse_yields_click_right() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x44, true, 300).unwrap().action,
            Some(Action::ClickRight)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Right, Dir::Down), (Button::Right, Dir::Up)]
        );
    }

    #[test]
    fn t09_w_in_mouse_yields_scroll_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x57, true, 300).unwrap().action,
            Some(Action::ScrollUp)
        );
        assert_eq!(hook.out().scrolls, vec![(0, 1)]);
    }

    #[test]
    fn t10_s_in_mouse_yields_scroll_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x53, true, 300).unwrap().action,
            Some(Action::ScrollDown)
        );
        assert_eq!(hook.out().scrolls, vec![(0, -1)]);
    }

    #[test]
    fn t11_tick_moves_cursor() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x4A, true, 300).unwrap(); // J
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);
    }

    #[test]
    fn t12_esc_exits_mouse_layer() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x1B, true, 300).unwrap(); // Esc
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t13_capslock_release_exits_mouse() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        hook.process_key(0x14, false, 400).unwrap(); // CapsLock release
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t14_u_in_mouse_yields_speed_down() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x55, true, 300).unwrap().action,
            Some(Action::SpeedDown)
        );
    }

    #[test]
    fn t15_o_in_mouse_yields_speed_up() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        assert_eq!(
            hook.process_key(0x4F, true, 300).unwrap().action,
            Some(Action::SpeedUp)
        );
    }

    #[derive(Default)]
    struct RecorderOverlay {
        shows: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
        hides: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl OverlayBackend for RecorderOverlay {
        fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
            self.shows.lock().unwrap().push(frame.level);
            Ok(())
        }

        fn hide(&mut self) -> Result<(), String> {
            *self.hides.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FrameRecord {
        level: u8,
        one: usize,
        two: usize,
    }

    #[derive(Default)]
    struct RecordingOverlay {
        frames: std::sync::Arc<std::sync::Mutex<Vec<FrameRecord>>>,
    }

    impl OverlayBackend for RecordingOverlay {
        fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
            self.frames.lock().unwrap().push(FrameRecord {
                level: frame.level,
                one: frame
                    .cells
                    .iter()
                    .filter(|cell| cell.label.chars().count() == 1)
                    .count(),
                two: frame
                    .cells
                    .iter()
                    .filter(|cell| cell.label.chars().count() == 2)
                    .count(),
            });
            Ok(())
        }

        fn hide(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn t19_grid_overlay_shows_each_level_then_hides() {
        use clickless_core::grid::GridConfig;

        let shows = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let hides = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());
        hook.set_overlay(Box::new(RecorderOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        enter_mouse(&mut hook);
        assert!(shows.lock().unwrap().is_empty());

        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid level 1
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        // Frames queue in order: level 1 cannot be silently replaced by
        // level 2 before presentation.
        hook.flush_overlay().unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1]);
        hook.flush_overlay().unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);

        hook.process_key(0x1B, true, 500).unwrap(); // Esc leaves grid
        hook.flush_overlay().unwrap();
        assert_eq!(*hides.lock().unwrap(), 1);
    }

    #[test]
    fn t18_grid_drag_holds_button_until_leader_release() {
        use clickless_core::grid::GridConfig;

        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                drag_after_select: true,
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );

        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x4B, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x4B, false, 600).unwrap(); // release -> drag start

        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);

        hook.process_key(0x4A, true, 700).unwrap(); // J drags right
        hook.tick(100).unwrap();
        assert_eq!(hook.out().moves, vec![(30, 0)]);

        hook.process_key(0x14, false, 800).unwrap(); // leader release
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t17_grid_release_moves_absolutely_then_left_clicks() {
        use clickless_core::grid::GridConfig;

        let mut bindings = HashMap::new();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            bindings,
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );

        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid
        hook.process_key(0x4B, true, 400).unwrap(); // K -> level 2
        hook.process_key(0x4B, true, 500).unwrap(); // K -> centre subcell
        hook.process_key(0x4B, false, 600).unwrap(); // release -> click

        assert_eq!(hook.out().abs, vec![(959, 540), (959, 540)]);
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    #[test]
    fn t16_with_config_applies_custom_leader_and_bindings() {
        let mut custom_bindings = HashMap::new();
        custom_bindings.insert(LogicalKey::H, Action::ClickLeft);
        let motion = MotionConfig {
            start_speed_px_s: 500,
            max_speed_px_s: 2000,
            ramp_ms: 250,
        };
        let mut hook =
            WindowsHook::with_config(MockOut::new(), LogicalKey::Space, custom_bindings, motion);

        let _ = hook.process_key(0x20, true, 0); // Space press
        let _ = hook.process_key(0x20, true, 200); // poll past threshold
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);

        assert_eq!(
            hook.process_key(0x48, true, 300).unwrap().action,
            Some(Action::ClickLeft)
        );
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
    }

    // consumed-event suppression (Prompt 1)

    use clickless_core::Outcome;
    use clickless_core::grid::GridConfig;

    struct FailingOverlay;

    impl OverlayBackend for FailingOverlay {
        fn show(&mut self, _frame: &OverlayFrame) -> Result<(), String> {
            Err("overlay show failed".to_string())
        }

        fn hide(&mut self) -> Result<(), String> {
            Err("overlay hide failed".to_string())
        }
    }

    fn dense_grid_hook() -> WindowsHook<MockOut> {
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::dense());
        hook
    }

    fn prepare_dense_level2_sequence() -> (
        WindowsHook<MockOut>,
        std::sync::Arc<std::sync::Mutex<Vec<FrameRecord>>>,
    ) {
        let frames = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut hook = dense_grid_hook();
        hook.set_overlay(Box::new(RecordingOverlay {
            frames: frames.clone(),
        }));
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // grid level 1
        hook.process_key(0x4B, true, 400).unwrap(); // K column
        hook.process_key(0x4B, false, 410).unwrap();
        hook.process_key(0x4B, true, 420).unwrap(); // K row: required subgrid
        (hook, frames)
    }

    #[test]
    fn t22_unmapped_vk_and_printscreen_pass_through() {
        let mut hook = WindowsHook::new(MockOut::new());
        assert_eq!(hook.process_key(0x0D, true, 0).unwrap(), Outcome::PASS);
        assert_eq!(hook.process_key(0x2C, true, 10).unwrap(), Outcome::PASS); // PrintScreen
        assert_eq!(hook.process_key(0x2C, false, 20).unwrap(), Outcome::PASS);
    }

    #[test]
    fn t23_printscreen_in_grid_passes_without_state_change() {
        let mut hook = dense_grid_hook();
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // Space -> grid level 1
        let before = hook.sm().grid_overlay();
        assert!(before.is_some());
        assert_eq!(hook.process_key(0x2C, true, 400).unwrap(), Outcome::PASS);
        assert_eq!(hook.process_key(0x2C, false, 410).unwrap(), Outcome::PASS);
        assert_eq!(hook.sm().grid_overlay(), before);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Grid);
    }

    #[test]
    fn t24_unbound_key_in_mouse_passes_without_action() {
        let mut bindings = HashMap::new();
        bindings.insert(LogicalKey::H, Action::ClickLeft);
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            bindings,
            MotionConfig::default(),
        );
        enter_mouse(&mut hook);
        let outcome = hook.process_key(0x4A, true, 300).unwrap(); // J mapped, unbound
        assert_eq!(outcome, Outcome::PASS);
        assert!(hook.out().moves.is_empty());
    }

    #[test]
    fn t25_bound_key_in_mouse_is_consumed() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        let outcome = hook.process_key(0x4A, true, 300).unwrap();
        assert!(outcome.consumed);
        assert_eq!(outcome.action, Some(Action::MoveRight));
    }

    #[test]
    fn t26_win_shift_s_chord_passes_and_tracking_clears() {
        let mut hook = dense_grid_hook();
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // grid level 1
        let before = hook.sm().grid_overlay();

        assert_eq!(hook.process_key(0x5B, true, 400).unwrap(), Outcome::PASS); // LWin down
        assert_eq!(hook.process_key(0x53, true, 410).unwrap(), Outcome::PASS); // S blocked by the chord
        assert_eq!(hook.sm().grid_overlay(), before);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Grid);

        assert_eq!(hook.process_key(0x5B, false, 420).unwrap(), Outcome::PASS); // LWin up
        let outcome = hook.process_key(0x53, true, 430).unwrap();
        assert!(outcome.consumed, "S must bind again after Win releases");
    }

    #[test]
    fn t27_leader_events_are_consumed_and_exit() {
        let mut hook = WindowsHook::new(MockOut::new());
        assert!(hook.process_key(0x14, true, 0).unwrap().consumed);
        assert!(hook.process_key(0x14, true, 200).unwrap().consumed); // promotes
        assert!(hook.process_key(0x14, false, 400).unwrap().consumed); // exits
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t28_esc_in_mouse_is_consumed_and_exits() {
        let mut hook = WindowsHook::new(MockOut::new());
        enter_mouse(&mut hook);
        let outcome = hook.process_key(0x1B, true, 300).unwrap();
        assert!(outcome.consumed);
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    #[test]
    fn t29_overlay_presentation_is_deferred_to_flush() {
        let shows = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let hides = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());
        hook.set_overlay(Box::new(RecorderOverlay {
            shows: shows.clone(),
            hides: hides.clone(),
        }));

        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // grid level 1 queued
        assert!(
            shows.lock().unwrap().is_empty(),
            "callback path must not present"
        );

        hook.flush_overlay().unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1]);

        hook.process_key(0x4B, true, 400).unwrap(); // level 2 queued
        assert_eq!(*shows.lock().unwrap(), vec![1]);
        hook.flush_overlay().unwrap();
        assert_eq!(*shows.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn t33_fast_nested_key_waits_until_subgrid_is_presented() {
        let (mut hook, frames) = prepare_dense_level2_sequence();
        let abs_before = hook.out().abs.len();

        // Fast third key arrives before the event loop has presented level 2.
        let outcome = hook.process_key(0x51, true, 430).unwrap(); // Q nested cell
        assert!(outcome.consumed);
        assert!(outcome.action.is_none());
        assert_eq!(
            hook.out().abs.len(),
            abs_before,
            "nested MoveTo overtook overlay"
        );

        hook.flush_overlay().unwrap();
        assert_eq!(
            frames.lock().unwrap().as_slice(),
            &[FrameRecord {
                level: 1,
                one: 0,
                two: 300,
            }]
        );
        assert_eq!(
            hook.out().abs.len(),
            abs_before,
            "level 1 flush cannot drain nested input"
        );

        hook.flush_overlay().unwrap();
        assert_eq!(
            frames.lock().unwrap().as_slice(),
            &[
                FrameRecord {
                    level: 1,
                    one: 0,
                    two: 300,
                },
                FrameRecord {
                    level: 1,
                    one: 0,
                    two: 30,
                }
            ]
        );
        assert_eq!(
            hook.out().abs.len(),
            abs_before,
            "bank-narrow flush cannot drain nested input"
        );

        hook.flush_overlay().unwrap();
        assert_eq!(
            frames.lock().unwrap().as_slice(),
            &[
                FrameRecord {
                    level: 1,
                    one: 0,
                    two: 300,
                },
                FrameRecord {
                    level: 1,
                    one: 0,
                    two: 30,
                },
                FrameRecord {
                    level: 2,
                    one: 30,
                    two: 299,
                },
            ]
        );
        assert_eq!(
            hook.out().abs.len(),
            abs_before + 1,
            "deferred nested press must run after level 2"
        );
    }

    #[test]
    fn t34_batched_dense_replay_presents_subgrid_before_nested_selection() {
        for _ in 0..1_000 {
            let (mut hook, frames) = prepare_dense_level2_sequence();
            let abs_before = hook.out().abs.len();
            hook.process_key(0x51, true, 430).unwrap(); // Q nested
            hook.process_key(0x51, false, 440).unwrap(); // release, also deferred
            for _ in 0..4 {
                hook.flush_overlay().unwrap();
            }
            assert!(
                hook.out().abs.len() > abs_before,
                "deferred nested input never ran"
            );
            let frames = frames.lock().unwrap();
            let level2 = frames
                .iter()
                .position(|frame| frame.level == 2 && frame.one == 30 && frame.two == 299);
            assert!(level2.is_some(), "required level 2 frame was not presented");
            assert_eq!(
                hook.out().buttons.len(),
                2,
                "deferred release must click after level 2"
            );
        }
    }

    #[test]
    fn t30_flush_surfaces_overlay_show_error() {
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, GridConfig::default());
        hook.set_overlay(Box::new(FailingOverlay));
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap(); // queue level 1
        let err = hook.flush_overlay().unwrap_err();
        assert_eq!(err, "overlay show failed");
    }

    #[test]
    fn t31_release_capture_releases_held_drag_button() {
        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(
            1920,
            1080,
            GridConfig {
                drag_after_select: true,
                auto_free_mode_after_move: false,
                ..GridConfig::default()
            },
        );
        enter_mouse(&mut hook);
        hook.process_key(0x20, true, 300).unwrap();
        hook.process_key(0x4B, true, 400).unwrap();
        hook.process_key(0x4B, true, 500).unwrap();
        hook.process_key(0x4B, false, 600).unwrap(); // drag started, button held
        assert_eq!(hook.out().buttons, vec![(Button::Left, Dir::Down)]);
        hook.release_capture();
        assert_eq!(
            hook.out().buttons,
            vec![(Button::Left, Dir::Down), (Button::Left, Dir::Up)]
        );
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
    }

    // Apply-to-engine (Prompt 3)

    #[test]
    fn t32_apply_config_swaps_leader_bindings_grid_and_theme() {
        use clickless_core::grid::{GridConfig, Rect};

        let mut config = Config::default();
        config.settings.leader = LogicalKey::Space;
        config.mouse_bindings.clear();
        config
            .mouse_bindings
            .insert(LogicalKey::H, Action::ClickLeft);
        config.grid = GridConfig::default(); // simple 3x3
        config.theme.panel = (10, 20, 30);

        let mut hook = WindowsHook::with_config(
            MockOut::new(),
            LogicalKey::CapsLock,
            clickless_core::default_bindings(),
            MotionConfig::default(),
        );
        hook.set_monitors(vec![Rect::new(0, 0, 1280, 720)]);
        hook.apply_config(config).unwrap();

        // Applying a configuration exits any active layer.
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
        // The old leader no longer arms capture.
        hook.process_key(0x14, true, 0).unwrap();
        hook.process_key(0x14, true, 300).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Initial);
        // The new leader does.
        hook.process_key(0x20, true, 500).unwrap();
        hook.process_key(0x20, true, 700).unwrap();
        assert_eq!(hook.sm().layer(), clickless_core::Layer::Mouse);
        // The new binding is live.
        let outcome = hook.process_key(0x48, true, 800).unwrap();
        assert_eq!(outcome.action, Some(Action::ClickLeft));
        // Grid uses the applied monitor rect: the simple 3x3 grid over
        // 1280x720 tiles from (0,0), so the first cell is 426x240.
        assert!(hook.sm().grid_overlay().is_none());
        hook.show_grid(true).unwrap();
        let frame = hook.sm().grid_overlay().unwrap();
        assert_eq!(frame.cells[0].rect.width, 426);
        assert_eq!(frame.cells[0].rect.height, 240);
    }

    #[test]
    fn t35_settings_focus_suspension_restores_previous_enabled_state() {
        let mut hook = WindowsHook::new(MockOut::new());
        assert!(!hook.sm().is_paused());

        hook.suspend_for_settings_focus(true).unwrap();
        assert!(hook.sm().is_paused());
        hook.suspend_for_settings_focus(false).unwrap();
        assert!(!hook.sm().is_paused());

        hook.set_paused(true).unwrap();
        hook.suspend_for_settings_focus(true).unwrap();
        assert!(hook.sm().is_paused());
        hook.suspend_for_settings_focus(false).unwrap();
        assert!(hook.sm().is_paused());
    }

    #[test]
    fn t36_apply_config_enabled_false_pauses_runtime() {
        let mut hook = WindowsHook::new(MockOut::new());
        let config = Config {
            enabled: false,
            ..Config::default()
        };
        hook.apply_config(config).unwrap();
        assert!(hook.sm().is_paused());

        let config = Config {
            enabled: true,
            ..Config::default()
        };
        hook.apply_config(config).unwrap();
        assert!(!hook.sm().is_paused());
    }

    #[test]
    fn t37_apply_config_propagates_hold_ms_and_scroll_step() {
        let mut hook = WindowsHook::new(MockOut::new());
        let mut config = Config::default();
        config.settings.hold_ms = 350;
        config.settings.scroll_step = 3;
        hook.apply_config(config).unwrap();
        assert_eq!(hook.sm().hold_ms(), 350);

        // Scroll actions use the applied step.
        hook.execute(Action::ScrollUp).unwrap();
        hook.execute(Action::ScrollDown).unwrap();
        assert_eq!(hook.out().scrolls, vec![(0, 3), (0, -3)]);
    }
}
