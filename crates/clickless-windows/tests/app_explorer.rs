//! App-rebuild 19: whole-app bounded state explorer (test infrastructure).
//!
//! Test-only model over production public APIs: `StateMachine`,
//! `WindowsHook` (process_key/flush_overlay/set_paused/apply_config/
//! show_grid/suspend_for_settings_focus), `SettingsEditor` and `Config`.
//! No private Win32 procedures, no pixel coordinates, no real pointer.

use clickless_backend_api::{Button, Dir, OutputBackend, OverlayBackend};
use clickless_config::Config;
use clickless_core::grid::{GridConfig, OverlayFrame};
use clickless_core::{Action, Layer, LogicalKey, MotionConfig};
use clickless_windows::WindowsHook;
use clickless_windows::settings_editor::{SettingsEditor, fields_from_config};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

// VK codes used by the explorer. All map through the real scancode table
// except PrintScreen (0x2C) and Win (0x5B), which the hook passes through.
const VK_LEADER: u32 = 0x14;
const VK_SPACE: u32 = 0x20;
const VK_K: u32 = 0x4B;
const VK_Q: u32 = 0x51;
const VK_Z: u32 = 0x5A;
#[allow(dead_code)]
const VK_J: u32 = 0x4A;
#[allow(dead_code)]
const VK_ESC: u32 = 0x1B;
#[allow(dead_code)]
const VK_BACKSPACE: u32 = 0x08;
#[allow(dead_code)]
const VK_PRINTSCREEN: u32 = 0x2C;
#[allow(dead_code)]
const VK_WIN: u32 = 0x5B;
#[allow(dead_code)]
const VK_S: u32 = 0x53;

static APP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct OutShared {
    abs: Vec<(i32, i32)>,
    downs: u32,
    ups: u32,
    fail_next: bool,
}

#[derive(Clone, Default)]
struct FaultOut {
    s: Arc<Mutex<OutShared>>,
}

impl FaultOut {
    fn take_fail(&self) -> bool {
        let mut s = self.s.lock().unwrap();
        if s.fail_next {
            s.fail_next = false;
            return true;
        }
        false
    }
}

impl OutputBackend for FaultOut {
    fn move_rel(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if self.take_fail() {
            return Err("injected move failure".to_string());
        }
        let _ = (dx, dy);
        Ok(())
    }

    fn move_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
        if self.take_fail() {
            return Err("injected move failure".to_string());
        }
        self.s.lock().unwrap().abs.push((x, y));
        Ok(())
    }

    fn button(&mut self, b: Button, d: Dir) -> Result<(), String> {
        if self.take_fail() {
            return Err("injected button failure".to_string());
        }
        let mut s = self.s.lock().unwrap();
        match d {
            Dir::Down => s.downs += 1,
            Dir::Up => s.ups += 1,
        }
        let _ = b;
        Ok(())
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if self.take_fail() {
            return Err("injected scroll failure".to_string());
        }
        let _ = (dx, dy);
        Ok(())
    }
}

#[derive(Default)]
struct OverlayShared {
    /// Currently presented level; None means hidden.
    shown: Option<u8>,
    shows: u32,
    hides: u32,
    hide_errs: u32,
    /// A hide attempt failed while this show cycle was open.
    open_failed_hide: bool,
    fail_next: bool,
}

#[derive(Clone, Default)]
struct FaultOverlay {
    s: Arc<Mutex<OverlayShared>>,
}

impl OverlayBackend for FaultOverlay {
    fn show(&mut self, frame: &OverlayFrame) -> Result<(), String> {
        let mut s = self.s.lock().unwrap();
        if s.fail_next {
            s.fail_next = false;
            return Err("injected overlay show failure".to_string());
        }
        s.shown = Some(frame.level);
        s.shows += 1;
        s.open_failed_hide = false;
        Ok(())
    }

    fn hide(&mut self) -> Result<(), String> {
        let mut s = self.s.lock().unwrap();
        if s.fail_next {
            s.fail_next = false;
            s.hide_errs += 1;
            if s.shown.is_some() {
                s.open_failed_hide = true;
            }
            return Err("injected overlay hide failure".to_string());
        }
        s.shown = None;
        s.hides += 1;
        Ok(())
    }
}

/// Canonical observable contract state. Only production-public reads feed
/// it, so two apps with equal snapshots are interchangeable to an owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Snap {
    layer: u8,
    armed: bool,
    /// Capped hold time: promotion only tests the 200 ms threshold, so
    /// values above 250 behave identically and stay one bucket.
    hold_ms: u16,
    paused: bool,
    grid_level: u8,
    /// Overlay cell counts by label length. Prefix narrowing changes the
    /// frame while the level stays 1, so the level alone is blind to it.
    one_cells: u16,
    two_cells: u16,
    overlay_shown: u8,
    held_button: bool,
    dirty: bool,
    focus: bool,
    applied_changed: bool,
    invalid_seen: bool,
    error_seen: bool,
}

#[derive(Clone, Copy, Debug)]
enum Ev {
    Key(u32, bool),
    Wait(u64),
    Tick,
    Flush,
    Pause(bool),
    Show(bool),
    Focus(bool),
    EditOk,
    EditBad,
    ApplyOk,
    ApplyFail,
    SaveOk,
    SaveBad,
    Cancel,
    FailOut,
    FailOverlay,
}

struct App {
    hook: WindowsHook<FaultOut>,
    out: FaultOut,
    overlay: FaultOverlay,
    editor: SettingsEditor,
    focused: bool,
    user_paused: bool,
    now_ms: u64,
    applied_base_speed: u64,
    invalid_base: u64,
    error_seen: bool,
    save_dir: Option<std::path::PathBuf>,
    save_path: Option<std::path::PathBuf>,
    saved_bytes: Vec<u8>,
}

impl App {
    fn new(grid: GridConfig) -> Self {
        let out = FaultOut::default();
        let overlay = FaultOverlay::default();
        let mut bindings = clickless_core::default_bindings();
        bindings.insert(LogicalKey::Space, Action::EnterGrid);
        let mut hook = WindowsHook::with_config(
            out.clone(),
            LogicalKey::CapsLock,
            bindings,
            MotionConfig::default(),
        );
        hook.sm_mut().enable_grid(1920, 1080, grid);
        hook.set_monitors(vec![clickless_core::grid::Rect::new(0, 0, 1920, 1080)]);
        hook.set_overlay(Box::new(FaultOverlay {
            s: overlay.s.clone(),
        }));
        let config = Config::default();
        let applied_base_speed = config.settings.start_speed_px_s;
        Self {
            hook,
            out,
            overlay,
            editor: SettingsEditor::new(config),
            focused: false,
            user_paused: false,
            now_ms: 0,
            applied_base_speed,
            invalid_base: 0,
            error_seen: false,
            save_dir: None,
            save_path: None,
            saved_bytes: Vec::new(),
        }
    }

    /// Save directory is created lazily: most explorer nodes never save,
    /// and BFS replays thousands of apps per second.
    fn ensure_save(&mut self) {
        if self.save_dir.is_none() {
            let id = APP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "clickless-explorer-{}-{}",
                std::process::id(),
                id
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("settings.toml");
            let bytes = Config::default().to_toml().into_bytes();
            std::fs::write(&path, &bytes).unwrap();
            self.saved_bytes = bytes;
            self.save_path = Some(path);
            self.save_dir = Some(dir);
        }
    }

    fn key(&mut self, vk: u32, down: bool) -> Result<(), String> {
        self.now_ms += 10;
        let now = self.now_ms;
        self.hook.process_key(vk, down, now).map(|_| ())
    }

    fn advance(&mut self, ms: u64) {
        self.now_ms += ms;
    }

    fn snapshot(&self) -> Snap {
        let layer = match self.hook.sm().layer() {
            Layer::Initial => 0,
            Layer::Mouse => 1,
            Layer::Grid => 2,
        };
        let frame = self.hook.sm().grid_overlay();
        let grid_level = frame.as_ref().map(|f| f.level).unwrap_or(0);
        let (one_cells, two_cells) = frame
            .as_ref()
            .map(|f| {
                (
                    f.cells
                        .iter()
                        .filter(|c| c.label.chars().count() == 1)
                        .count(),
                    f.cells
                        .iter()
                        .filter(|c| c.label.chars().count() == 2)
                        .count(),
                )
            })
            .unwrap_or((0, 0));
        let overlay_shown = self.overlay.s.lock().unwrap().shown.unwrap_or(0);
        let o = self.out.s.lock().unwrap();
        Snap {
            layer,
            armed: self.hook.sm().is_leader_armed(),
            hold_ms: self
                .hook
                .sm()
                .leader_hold_ms(self.now_ms)
                .map(|ms| ms.min(250) as u16)
                .unwrap_or(0),
            paused: self.hook.sm().is_paused(),
            grid_level,
            one_cells: one_cells.min(999) as u16,
            two_cells: two_cells.min(999) as u16,
            overlay_shown,
            held_button: o.downs > o.ups,
            dirty: self.editor.is_dirty(),
            focus: self.focused,
            applied_changed: self.editor.applied().settings.start_speed_px_s
                != self.applied_base_speed,
            invalid_seen: self.hook.sm().invalid_presses() != self.invalid_base,
            error_seen: self.error_seen,
        }
    }

    /// Whole-app invariants after every event, not only at sequence end.
    fn check(&self, ctx: &str) {
        let snap = self.snapshot();
        let o = self.out.s.lock().unwrap();
        assert!(
            o.downs <= o.ups + 1,
            "{ctx}: at most one held button (downs={} ups={})",
            o.downs,
            o.ups
        );
        if snap.layer == 0 {
            assert!(
                snap.grid_level == 0,
                "{ctx}: initial layer must have no grid overlay"
            );
            assert!(
                !snap.held_button,
                "{ctx}: initial layer must hold no button"
            );
        }
        if snap.layer == 2 {
            assert!(
                snap.grid_level != 0,
                "{ctx}: grid layer with inactive navigator (audit P0-2)"
            );
        }
        if snap.paused {
            assert!(
                snap.grid_level == 0,
                "{ctx}: paused runtime must show no grid state"
            );
            assert!(!snap.held_button, "{ctx}: paused runtime holds a button");
        }
        assert_eq!(
            self.editor.is_dirty(),
            self.editor.draft() != self.editor.applied(),
            "{ctx}: dirty flag must match draft/applied"
        );
        assert_eq!(
            self.hook.sm().is_paused(),
            self.user_paused || self.focused,
            "{ctx}: pause must equal user-pause or settings suspension"
        );
        drop(o);
    }

    /// End-of-sequence cleanup mirroring production shutdown: release
    /// capture, present the final frame, then demand full balance. Hiding
    /// is best-effort: if the hide itself fails, the model requires a
    /// demonstrated exhausted attempt, not hidden pixels. The real loop
    /// exits the process on such errors and the OS removes its windows.
    fn finish(&mut self, ctx: &str) {
        self.hook.release_capture();
        let _ = self.hook.flush_overlay();
        self.check(ctx);
        let o = self.out.s.lock().unwrap();
        assert_eq!(
            o.downs, o.ups,
            "{ctx}: every button-down needs its button-up"
        );
        drop(o);
        // Best-effort hide: a failed hide during shutdown leaves pixels up
        // while the hook believes them hidden. Accept only when the open
        // show cycle records a failed hide attempt.
        if self.overlay.s.lock().unwrap().shown.is_some() {
            assert!(
                self.overlay.s.lock().unwrap().open_failed_hide,
                "{ctx}: overlay stuck visible with no failed hide attempt"
            );
            self.error_seen = true;
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(dir) = self.save_dir.take() {
            std::fs::remove_dir_all(dir).ok();
        }
    }
}

/// Applies one canonical event. Hook errors follow production hygiene: the
/// first error releases capture and is recorded; the model keeps running.
fn step(app: &mut App, ev: Ev) {
    match ev {
        Ev::Key(vk, down) => {
            if app.key(vk, down).is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
        }
        Ev::Wait(ms) => app.advance(ms),
        Ev::Tick => {
            if app.hook.tick(10).is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
        }
        Ev::Flush => {
            if app.hook.flush_overlay().is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
        }
        Ev::Pause(p) => {
            app.user_paused = p;
            if app.hook.set_paused(p).is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
        }
        Ev::Show(show) => {
            if app.hook.show_grid(show).is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
        }
        Ev::Focus(focused) => {
            app.focused = focused;
            if app.hook.suspend_for_settings_focus(focused).is_err() {
                app.error_seen = true;
                app.hook.release_capture();
            }
            if !focused {
                // Focus-leave restores the paused intent inside the hook;
                // the tray model follows the authoritative state.
                app.user_paused = app.hook.sm().is_paused();
            }
        }
        Ev::EditOk => {
            let mut fields = fields_from_config(app.editor.draft());
            fields.start_speed_px_s = "400".to_string();
            app.editor.edit(&fields).unwrap();
        }
        Ev::EditBad => {
            let mut fields = fields_from_config(app.editor.draft());
            fields.start_speed_px_s = "abc".to_string();
            assert!(app.editor.edit(&fields).is_err());
        }
        Ev::ApplyOk => {
            let applied = app
                .editor
                .apply_to_runtime(|_| Ok::<(), String>(()))
                .unwrap();
            let _ = app.hook.apply_config(applied.clone());
            // The applied config is authoritative for paused state; the
            // tray model follows it (display staleness is a native check).
            app.user_paused = app.hook.sm().is_paused();
        }
        Ev::ApplyFail => {
            let before = app.editor.applied().clone();
            let hook_before = app.hook.current_config();
            assert!(
                app.editor
                    .apply_to_runtime(|_| Err::<(), String>("boom".into()))
                    .is_err()
            );
            assert_eq!(app.editor.applied(), &before);
            assert_eq!(app.hook.current_config(), hook_before);
        }
        Ev::SaveOk => {
            app.ensure_save();
            let applied = app
                .editor
                .apply_to_runtime(|_| Ok::<(), String>(()))
                .unwrap();
            let _ = app.hook.apply_config(applied.clone());
            app.user_paused = app.hook.sm().is_paused();
            app.editor.save(app.save_path.as_ref().unwrap()).unwrap();
            app.saved_bytes = std::fs::read(app.save_path.as_ref().unwrap()).unwrap();
        }
        Ev::SaveBad => {
            app.ensure_save();
            // Parent is a regular file: no atomic save can create a directory
            // there, so the write must fail and the previous bytes must stay.
            // A merely missing directory is no longer an error: save creates
            // the parent chain on purpose.
            let blocker = app
                .save_path
                .as_ref()
                .unwrap()
                .parent()
                .unwrap()
                .join("blocker-file");
            std::fs::write(&blocker, b"x").unwrap();
            let bad = blocker.join("s.toml");
            assert!(app.editor.save(&bad).is_err());
            assert_eq!(
                std::fs::read(app.save_path.as_ref().unwrap()).unwrap(),
                app.saved_bytes
            );
        }
        Ev::Cancel => app.editor.cancel(),
        Ev::FailOut => app.out.s.lock().unwrap().fail_next = true,
        Ev::FailOverlay => app.overlay.s.lock().unwrap().fail_next = true,
    }
    app.check(&format!("after {ev:?}"));
}

fn enter_mouse(app: &mut App) {
    step(app, Ev::Key(VK_LEADER, true));
    step(app, Ev::Wait(250));
    step(app, Ev::Key(VK_LEADER, true));
    assert_eq!(app.hook.sm().layer(), Layer::Mouse);
}

fn default_grid_app() -> App {
    App::new(GridConfig {
        auto_free_mode_after_move: false,
        ..GridConfig::default()
    })
}

fn dense_app() -> App {
    App::new(GridConfig::dense())
}

// Named regressions. Each discovered minimal sequence becomes one of these
// before the fix lands.

#[test]
fn deferred_input_from_an_old_session_never_runs() {
    // Fast nested key deferred, then pause exits capture before any flush.
    // The dead session's queued frames and deferred keys must not leak:
    // no stale overlay presents, and the next session behaves exactly like
    // a fresh one.
    let mut app = dense_app();
    enter_mouse(&mut app);
    step(&mut app, Ev::Key(VK_SPACE, true));
    step(&mut app, Ev::Key(VK_K, true));
    step(&mut app, Ev::Key(VK_K, false));
    step(&mut app, Ev::Key(VK_K, true));
    step(&mut app, Ev::Key(VK_Q, true));
    step(&mut app, Ev::Key(VK_Q, false));
    step(&mut app, Ev::Pause(true));
    step(&mut app, Ev::Pause(false));
    step(&mut app, Ev::Flush);
    step(&mut app, Ev::Flush);
    // Session one moved to the parent center once; the stale Q press and
    // release are gone, so no click and no presented overlay remain.
    assert_eq!(app.out.s.lock().unwrap().abs.len(), 1);
    assert_eq!(app.overlay.s.lock().unwrap().shown, None);
    assert_eq!(app.hook.sm().layer(), Layer::Initial);
    // Fresh session behaves exactly like a first session.
    enter_mouse(&mut app);
    step(&mut app, Ev::Key(VK_SPACE, true));
    step(&mut app, Ev::Key(VK_K, true));
    step(&mut app, Ev::Key(VK_K, false));
    step(&mut app, Ev::Key(VK_K, true));
    step(&mut app, Ev::Flush);
    step(&mut app, Ev::Flush);
    step(&mut app, Ev::Flush);
    let presented = app.overlay.s.lock().unwrap().shown;
    assert_eq!(presented, Some(2), "fresh subgrid must present");
    let abs_before = app.out.s.lock().unwrap().abs.len();
    step(&mut app, Ev::Key(VK_Q, true));
    step(&mut app, Ev::Key(VK_Q, false));
    step(&mut app, Ev::Flush);
    step(&mut app, Ev::Flush);
    step(&mut app, Ev::Flush);
    let o = app.out.s.lock().unwrap();
    // Nested press moves once; release clicks at the target (its own move).
    assert_eq!(o.abs.len(), abs_before + 2, "exactly one nested move+click");
    assert_eq!(o.downs, 1, "exactly one button down");
    assert_eq!(o.ups, 1, "exactly one button up");
    drop(o);
    app.finish("stale-deferred");
}

#[test]
fn failed_runtime_apply_changes_nothing() {
    let mut app = default_grid_app();
    step(&mut app, Ev::EditOk);
    assert!(app.editor.is_dirty());
    step(&mut app, Ev::ApplyFail);
    assert!(app.editor.is_dirty(), "failed apply keeps the dirty draft");
    assert_eq!(
        app.editor.applied().settings.start_speed_px_s,
        app.applied_base_speed
    );
    assert_eq!(
        app.hook.current_config().settings.start_speed_px_s,
        app.applied_base_speed
    );
    app.finish("apply-fail");
}

#[test]
fn invalid_key_at_hook_level_never_reaches_output() {
    let mut app = dense_app();
    enter_mouse(&mut app);
    step(&mut app, Ev::Key(VK_SPACE, true));
    let before = app.hook.sm().grid_overlay();
    assert!(before.is_some());
    step(&mut app, Ev::Key(VK_Z, true));
    step(&mut app, Ev::Key(VK_Z, false));
    assert!(app.out.s.lock().unwrap().abs.is_empty());
    assert_eq!(app.hook.sm().grid_overlay(), before);
    app.finish("invalid-contained");
}

#[test]
fn hide_failure_at_shutdown_retries_then_accepts() {
    // Explorer find (fuzz seed 89, minimal [Show(true), FailOverlay]):
    // hiding during release_capture can fail. Cleanup must retry once;
    // buttons and layer must be clean regardless, and the failed attempt
    // is recorded instead of silently dropped.
    let mut app = default_grid_app();
    enter_mouse(&mut app);
    step(&mut app, Ev::Show(true));
    step(&mut app, Ev::FailOverlay);
    app.finish("hide-best-effort");
    assert_eq!(app.hook.sm().layer(), Layer::Initial);
    let o = app.out.s.lock().unwrap();
    assert_eq!(o.downs, o.ups);
    assert!(
        app.overlay.s.lock().unwrap().hide_errs >= 1,
        "the failed hide must be recorded"
    );
}

// ---- bounded exploration (part 2) ----

impl App {
    fn new_full(grid: GridConfig, enabled: bool) -> Self {
        let mut app = App::new(grid);
        if !enabled {
            let config = Config {
                enabled: false,
                ..Config::default()
            };
            app.hook.apply_config(config.clone()).unwrap();
            app.editor = SettingsEditor::new(config);
            // The applied config is authoritative from boot.
            app.user_paused = app.hook.sm().is_paused();
        }
        app
    }
}

/// BFS alphabet: every canonical input class, no file IO.
fn bfs_alphabet() -> Vec<Ev> {
    vec![
        Ev::Key(VK_LEADER, true),
        Ev::Key(VK_LEADER, false),
        Ev::Key(VK_SPACE, true),
        Ev::Key(VK_K, true),
        Ev::Key(VK_K, false),
        Ev::Key(VK_Q, true),
        Ev::Key(VK_Z, true),
        Ev::Key(VK_PRINTSCREEN, true),
        Ev::Wait(30),
        Ev::Wait(250),
        Ev::Tick,
        Ev::Flush,
        Ev::Pause(true),
        Ev::Pause(false),
        Ev::Show(true),
        Ev::Show(false),
        Ev::Focus(true),
        Ev::Focus(false),
        Ev::EditOk,
        Ev::EditBad,
        Ev::ApplyOk,
        Ev::ApplyFail,
        Ev::Cancel,
    ]
}

fn replay(grid: &GridConfig, enabled: bool, events: &[Ev]) -> App {
    let mut app = App::new_full(grid.clone(), enabled);
    for ev in events {
        step(&mut app, *ev);
    }
    app
}

/// Breadth-first explorer through `depth`, deduplicating canonical
/// snapshots. Returns (visited snapshots, applied transitions).
fn explore(
    grid: &GridConfig,
    enabled: bool,
    depth: usize,
    alphabet: &[Ev],
) -> (usize, usize, std::collections::HashSet<Snap>) {
    use std::collections::{HashSet, VecDeque};
    let mut seen = HashSet::new();
    // One revisit expansion per snapshot: time-only events (Wait) leave the
    // snapshot unchanged yet enable promotion on the next key. The first
    // revisit still expands; later ones prune. Invariants are checked on
    // every pop regardless.
    let mut revisited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(Vec::<Ev>::new());
    let mut transitions = 0usize;
    const STEP_CAP: usize = 500_000;
    while let Some(prefix) = queue.pop_front() {
        let mut app = replay(grid, enabled, &prefix);
        let snap = app.snapshot();
        let fresh = seen.insert(snap);
        if !fresh && !revisited.insert(snap) {
            continue;
        }
        if prefix.len() >= depth {
            app.finish("bfs-leaf");
            continue;
        }
        for ev in alphabet {
            let mut next = prefix.clone();
            next.push(*ev);
            transitions += 1;
            assert!(
                transitions <= STEP_CAP,
                "state explosion: blew the {STEP_CAP}-step budget"
            );
            queue.push_back(next);
        }
    }
    let states = seen.len();
    (states, transitions, seen)
}

fn covers(set: &std::collections::HashSet<Snap>, pred: impl FnMut(&Snap) -> bool) -> bool {
    set.iter().any(pred)
}

#[test]
fn bfs_covers_canonical_states() {
    let alphabet = bfs_alphabet();
    let simple = GridConfig {
        auto_free_mode_after_move: false,
        ..GridConfig::default()
    };
    let drag = GridConfig {
        auto_free_mode_after_move: false,
        drag_after_select: true,
        ..GridConfig::default()
    };
    let dense = GridConfig::dense();
    let mut total_states = 0;
    let mut total_transitions = 0;
    for (name, grid, enabled) in [
        ("simple", simple, true),
        ("drag", drag, true),
        ("dense", dense, true),
        ("disabled", GridConfig::dense(), false),
    ] {
        let (states, transitions, set) = explore(&grid, enabled, 7, &alphabet);
        println!("bfs {name}: {states} states, {transitions} transitions");
        total_states += states;
        total_transitions += transitions;
        if enabled {
            assert!(
                covers(&set, |s| s.layer == 1),
                "{name}: never reached Mouse"
            );
            assert!(
                covers(&set, |s| s.layer == 2 && s.grid_level == 1),
                "{name}: never reached grid level 1"
            );
            assert!(
                covers(&set, |s| s.grid_level == 2),
                "{name}: never reached grid level 2"
            );
            assert!(covers(&set, |s| s.paused), "{name}: never paused");
            assert!(covers(&set, |s| s.focus), "{name}: never suspended");
            assert!(covers(&set, |s| s.dirty), "{name}: never dirty");
        } else {
            assert!(
                covers(&set, |s| s.paused),
                "{name}: disabled boot must pause"
            );
        }
    }
    println!("bfs total: {total_states} states, {total_transitions} transitions");
    assert!(
        total_states >= 60,
        "explorer degenerated: {total_states} states"
    );
}

fn combo_grid(index: usize) -> GridConfig {
    let mut grid = if index & 1 != 0 {
        GridConfig::dense()
    } else {
        GridConfig::default()
    };
    grid.nudge_enabled = index & 2 != 0;
    grid.drag_after_select = index & 4 != 0;
    grid.auto_free_mode_after_move = index & 8 != 0;
    grid
}

#[test]
fn combo_sweep_all_config_pairs() {
    // Full cross product of dense/nudge/drag/auto-free/enabled: 32 combos,
    // shallow depth. Every pair of flags co-occurs in several combos.
    let alphabet = bfs_alphabet();
    let mut total_states = 0;
    for index in 0..32 {
        let grid = combo_grid(index);
        let enabled = index & 16 != 0;
        let (states, transitions, _) = explore(&grid, enabled, 3, &alphabet);
        total_states += states;
        println!("combo {index:02}: {states} states, {transitions} transitions");
    }
    println!("combo total: {total_states} states");
    assert!(total_states >= 200, "combo sweep degenerated");
}

// ---- deterministic seeded fuzz ----

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const FUZZ_KEYS: [u32; 8] = [
    VK_LEADER,
    VK_J,
    VK_SPACE,
    VK_K,
    VK_Q,
    VK_Z,
    VK_ESC,
    VK_BACKSPACE,
];

fn fuzz_event(rng: &mut Rng, last_down: &mut Option<u32>) -> Ev {
    // Bias toward press-without-release, repeats and overlapping input.
    let roll = rng.below(100);
    match roll {
        0..30 => {
            let vk = FUZZ_KEYS[rng.below(FUZZ_KEYS.len() as u64) as usize];
            *last_down = Some(vk);
            Ev::Key(vk, true)
        }
        30..45 => {
            if let Some(vk) = *last_down {
                *last_down = None;
                Ev::Key(vk, false)
            } else {
                Ev::Key(VK_K, true)
            }
        }
        45..52 => Ev::Wait(if rng.below(2) == 0 { 30 } else { 250 }),
        52..60 => Ev::Tick,
        60..68 => Ev::Flush,
        68..71 => Ev::Pause(rng.below(2) == 0),
        71..74 => Ev::Show(rng.below(2) == 0),
        74..78 => Ev::Focus(rng.below(2) == 0),
        78..81 => Ev::EditOk,
        81..83 => Ev::EditBad,
        83..86 => Ev::ApplyOk,
        86..88 => Ev::ApplyFail,
        88 => {
            // Real atomic writes cost ~10 ms on Windows; keep them rare.
            // SaveBad (failed-write preservation) stays at 1%: it is cheap.
            if rng.below(10) == 0 {
                Ev::SaveOk
            } else {
                Ev::Tick
            }
        }
        89..90 => Ev::SaveBad,
        90..92 => Ev::Cancel,
        92..94 => Ev::FailOut,
        94..96 => Ev::FailOverlay,
        _ => {
            if rng.below(2) == 0 {
                Ev::Key(VK_WIN, true)
            } else {
                Ev::Key(VK_S, true)
            }
        }
    }
}

fn env_usize(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(fallback)
}

/// Runs one seed. Returns the failing event prefix, if any.
fn run_seed(seed: u64, events_per_seed: usize) -> Option<(usize, bool, Vec<Ev>, String)> {
    let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
    let grid_index = (seed % 16) as usize;
    let grid = combo_grid(grid_index);
    let enabled = seed % 32 >= 16;
    let mut events = Vec::with_capacity(events_per_seed);
    let mut last_down = None;
    for _ in 0..events_per_seed {
        events.push(fuzz_event(&mut rng, &mut last_down));
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut app = App::new_full(grid.clone(), enabled);
        for ev in &events {
            step(&mut app, *ev);
        }
        app.finish("fuzz-seed");
    }));
    match result {
        Ok(()) => None,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic".to_string());
            Some((grid_index, enabled, events, msg))
        }
    }
}

/// Greedy minimization: halve chunks until single events remain, then one
/// linear pass. Bounded so CI cannot hang.
fn minimize(grid_index: usize, enabled: bool, events: Vec<Ev>) -> Vec<Ev> {
    let grid = combo_grid(grid_index);
    let fails = |candidate: &[Ev]| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut app = App::new_full(grid.clone(), enabled);
            for ev in candidate {
                step(&mut app, *ev);
            }
            app.finish("minimize");
        }))
        .is_err()
    };
    let mut current = events;
    let mut chunk = current.len() / 2;
    let mut replays = 0;
    while chunk >= 1 && replays < 2000 {
        let mut i = 0;
        while i + chunk <= current.len() && replays < 2000 {
            let mut candidate = current[..i].to_vec();
            candidate.extend_from_slice(&current[i + chunk..]);
            replays += 1;
            if fails(&candidate) {
                current = candidate;
            } else {
                i += chunk;
            }
        }
        chunk /= 2;
    }
    current
}

#[test]
fn seeded_fuzz_reports_every_failure() {
    let seeds = env_usize("CLICKLESS_EXPLORER_SEEDS", 10_000);
    let per_seed = env_usize("CLICKLESS_EXPLORER_EVENTS", 1_000);
    let mut reports = Vec::new();
    for seed in 0..seeds as u64 {
        if let Some((grid_index, enabled, events, msg)) = run_seed(seed, per_seed) {
            let minimal = minimize(grid_index, enabled, events);
            let mut app = App::new_full(combo_grid(grid_index), enabled);
            for ev in &minimal {
                step(&mut app, *ev);
            }
            reports.push(format!(
                "seed {seed} config {grid_index}: {msg}\n  events: {minimal:?}\n  final: {:?}",
                app.snapshot()
            ));
            if reports.len() >= 5 {
                break;
            }
        }
    }
    assert!(
        reports.is_empty(),
        "explorer found {} failing seed(s):\n{}",
        reports.len(),
        reports.join("\n")
    );
    println!("fuzz: {seeds} seeds x {per_seed} events, zero failures");
}

#[cfg(windows)]
#[test]
fn single_instance_allows_exactly_one_owner() {
    use clickless_windows::lifecycle::SingleInstance;
    let first = SingleInstance::acquire();
    if first.is_err() {
        // Another Clickless runs on this machine (live owner session); the
        // negative path is still exercised by the reason string.
        return;
    }
    let guard = first.unwrap();
    assert!(SingleInstance::acquire().is_err());
    drop(guard);
    assert!(SingleInstance::acquire().is_ok());
}
