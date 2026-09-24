//! Native Settings window (Prompt 3): one scrollable page of native controls
//! driven by the pure `settings_editor` state machine.
//!
//! Division of labour: this module owns Win32 only. Every field parse,
//! validation rule and Apply/Save/Cancel decision lives in
//! `crate::settings_editor`. The window reads control text into `Fields`,
//! calls one editor method, and mirrors the result back to the controls.
//!
//! The window and its editor state are created and driven on the event-loop
//! thread only; a thread-local owns the editor and the runtime callback.

#[cfg(windows)]
pub mod win {
    use crate::settings_editor::{Fields, Section, SettingsEditor, fields_from_config};
    use std::cell::{Cell, RefCell};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateFontW, DEFAULT_CHARSET,
        DEFAULT_QUALITY, DeleteObject, EndPaint, FF_DONTCARE, FW_NORMAL, GetDC, GetTextFaceW,
        HBRUSH, HFONT, OUT_TT_PRECIS, PAINTSTRUCT, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE,
        RDW_UPDATENOW, RedrawWindow, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::UI::Controls::SetScrollInfo;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BringWindowToTop,
        CB_ADDSTRING, CB_FINDSTRINGEXACT, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN,
        CB_SETCURSEL, CBS_DROPDOWNLIST, CreateWindowExW, DefWindowProcW, DestroyWindow,
        EnumChildWindows, GWLP_USERDATA, GetClientRect, GetDlgItem, GetForegroundWindow, GetParent,
        GetScrollInfo, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IDNO,
        IDYES, IsDialogMessageW, IsWindowVisible, MB_ICONQUESTION, MB_YESNOCANCEL, MINMAXINFO, MSG,
        MessageBoxW, RegisterClassW, SB_BOTTOM, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP,
        SB_THUMBPOSITION, SB_THUMBTRACK, SB_TOP, SB_VERT, SCROLLINFO, SIF_ALL, SIF_PAGE, SIF_POS,
        SIF_RANGE, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE,
        SWP_NOCOPYBITS, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetForegroundWindow,
        SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, WM_CLOSE, WM_COMMAND,
        WM_DESTROY, WM_GETMINMAXINFO, WM_KEYDOWN, WM_MOUSEWHEEL, WM_MOVE, WM_PAINT, WM_SETFONT,
        WM_VSCROLL, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN,
        WS_EX_CONTROLPARENT, WS_EX_TOOLWINDOW, WS_MINIMIZEBOX, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
        WS_VSCROLL,
    };

    // Button check states (Winuser.h); not exported by windows-sys 0.61.
    // SendMessageW returns/wants `usize` here.
    const BST_UNCHECKED: usize = 0;
    const BST_CHECKED: usize = 1;
    const CHECKBOX_STYLE: u32 = WS_TABSTOP | BS_AUTOCHECKBOX as u32;

    const CLASS_NAME: &str = "ClicklessSettingsWindow";

    // Control IDs: stable handles for GetDlgItem/WM_COMMAND.
    const ID_LEADER: i32 = 101;
    const ID_START_SPEED: i32 = 102;
    const ID_MAX_SPEED: i32 = 103;
    const ID_RAMP: i32 = 104;
    const ID_LAYOUT: i32 = 105;
    const ID_NUDGE_ENABLE: i32 = 106;
    const ID_NUDGE_STEP: i32 = 107;
    const ID_DRAG: i32 = 108;
    const ID_AUTO_FREE: i32 = 109;
    const ID_PANEL: i32 = 110;
    const ID_PANEL_OPACITY: i32 = 111;
    const ID_BORDER: i32 = 112;
    const ID_HIGHLIGHT: i32 = 113;
    const ID_LABEL_COLOR: i32 = 114;
    const ID_POINTER: i32 = 115;
    const ID_LABEL_SIZE: i32 = 116;
    const ID_ENABLED: i32 = 117;
    const ID_HIGHLIGHT_OPACITY: i32 = 118;
    const ID_BORDER_PX: i32 = 119;
    const ID_MESSAGE: i32 = 120;
    const ID_APPLY: i32 = 121;
    const ID_SAVE: i32 = 122;
    const ID_CANCEL: i32 = 123;
    const ID_BINDING_ERROR: i32 = 124;
    const ID_GRID_ERROR: i32 = 125;
    const ID_GRID_ROWS: i32 = 126;
    const ID_GRID_COLS: i32 = 127;
    const ID_GRID_KEYS: i32 = 128;
    const ID_COLUMN_KEYS: i32 = 129;
    const ID_ROW_KEYS: i32 = 130;
    const ID_HOLD_MS: i32 = 131;
    const ID_SCROLL_STEP: i32 = 132;
    const ID_RESET_SETTINGS: i32 = 140;
    const ID_RESET_APPEARANCE: i32 = 141;
    const ID_RESET_BINDINGS: i32 = 142;
    const ID_RESET_GRID: i32 = 143;
    const ID_RESET_ALL: i32 = 144;
    const ID_SEARCH: i32 = 145;
    const ID_COPY_DIAG: i32 = 146;
    const ID_OPEN_LOG: i32 = 147;
    /// Window placement: fixed 820x640 (resizable later; the 680x520
    /// minimum in the brief applies then), persisted position only.
    const SETTINGS_WIDTH: i32 = 820;
    const SETTINGS_HEIGHT: i32 = 640;
    /// Minimum size from the brief, enforced on restore and (later) resize.
    const SETTINGS_MIN_WIDTH: i32 = 680;
    const SETTINGS_MIN_HEIGHT: i32 = 520;
    const ID_PRACTICE_AGAIN: i32 = 148;
    const ID_RESET_PAGE: i32 = 156;

    // Search groups double as sidebar pages, in content order. Every
    // content child is tagged with its group at creation so page switching
    // and the filter can hide and compact whole groups. The search box
    // itself lives on the top-level window and is never tagged.
    const GROUP_GENERAL: usize = 0;
    const GROUP_MOVEMENT: usize = 1;
    const GROUP_GRID: usize = 2;
    const GROUP_SHORTCUTS: usize = 3;
    const GROUP_APPEARANCE: usize = 4;
    const GROUP_ABOUT: usize = 5;
    const GROUP_COUNT: usize = 6;
    const NO_GROUP: usize = usize::MAX;
    // Edit-control notification (Winuser.h); not exported by windows-sys 0.61.
    const EN_CHANGE: u32 = 0x300;
    const EN_SETFOCUS: u32 = 0x100;
    // Button/combo notifications that mean "a value changed".
    const BN_CLICKED: u32 = 0;
    const CBN_SELCHANGE: u32 = 1;

    #[derive(Clone, Copy)]
    struct RegEntry {
        hwnd: HWND,
        group: usize,
        x: i32,
        y: i32,
        h: i32,
    }
    const DEFAULT_MOUSE_BINDING_SLOTS: usize = 11;
    const MOUSE_KEY_BASE: i32 = 200;
    const MOUSE_ACTION_BASE: i32 = 220;
    const MOUSE_RECORD_BASE: i32 = 240;
    const MOUSE_CLEAR_BASE: i32 = 260;
    const FOOTER_HEIGHT: i32 = 120;
    const ID_MOUSE_BINDINGS_LABEL: i32 = 900;
    const ID_KEY_LABEL: i32 = 901;
    const ID_ACTION_LABEL: i32 = 902;
    const ID_GRID_LABEL: i32 = 903;
    const ID_MOTION_LABEL: i32 = 905;
    const ID_APPEAR_LABEL: i32 = 906;
    const ID_ABOUT_LABEL: i32 = 908;
    const ID_MOVEMENT_LABEL: i32 = 909;
    const ID_VERSION_TEXT: i32 = 910;
    // Sidebar page buttons live on the top-level window, always visible.
    const ID_PAGE_GENERAL: i32 = 150;
    const ID_PAGE_MOVEMENT: i32 = 151;
    const ID_PAGE_GRID: i32 = 152;
    const ID_PAGE_SHORTCUTS: i32 = 153;
    const ID_PAGE_APPEARANCE: i32 = 154;
    const ID_PAGE_ABOUT: i32 = 155;
    const PAGE_IDS: [i32; GROUP_COUNT] = [
        ID_PAGE_GENERAL,
        ID_PAGE_MOVEMENT,
        ID_PAGE_GRID,
        ID_PAGE_SHORTCUTS,
        ID_PAGE_APPEARANCE,
        ID_PAGE_ABOUT,
    ];
    const PAGE_NAMES: [&str; GROUP_COUNT] = [
        "General",
        "Movement",
        "Grid",
        "Shortcuts",
        "Appearance",
        "About",
    ];
    const ID_INITIAL_BINDINGS_NOTE: i32 = 904;
    const ACTIONS: [&str; 11] = [
        "click_left",
        "click_right",
        "scroll_up",
        "scroll_down",
        "enter_grid",
        "move_left",
        "move_right",
        "move_up",
        "move_down",
        "speed_down",
        "speed_up",
    ];
    /// Human phrases shown in the action dropdown. The token stays the stored
    /// value; only the label reaches the user's eyes.
    const ACTION_LABELS: [&str; 11] = [
        "Left click",
        "Right click",
        "Scroll up",
        "Scroll down",
        "Enter grid",
        "Move left",
        "Move right",
        "Move up",
        "Move down",
        "Speed down",
        "Speed up",
    ];

    const LAYOUT_TOKENS: [&str; 2] = ["dense", "simple"];
    const LAYOUT_LABELS: [&str; 2] = ["Dense", "Simple"];

    /// Token stored in the draft -> phrase shown in the dropdown. Unknown
    /// tokens pass through so foreign config values still display.
    fn display_label(token: &str) -> &str {
        for (tokens, labels) in [
            (&ACTIONS[..], &ACTION_LABELS[..]),
            (&LAYOUT_TOKENS[..], &LAYOUT_LABELS[..]),
        ] {
            if let Some(index) = tokens.iter().position(|candidate| *candidate == token) {
                return labels[index];
            }
        }
        token
    }

    /// Phrase shown in the dropdown -> token stored in the draft.
    fn stored_token(label: &str) -> &str {
        for (tokens, labels) in [
            (&ACTIONS[..], &ACTION_LABELS[..]),
            (&LAYOUT_TOKENS[..], &LAYOUT_LABELS[..]),
        ] {
            if let Some(index) = labels.iter().position(|candidate| *candidate == label) {
                return tokens[index];
            }
        }
        label
    }

    /// Callback holding the draft apply. `Send` because the WinUI 3 shell
    /// runs it from a WinRT delegate.
    type ApplyCallback = Box<dyn FnMut(&clickless_config::Config) -> Result<(), String> + Send>;

    thread_local! {
        static EDITOR: RefCell<Option<SettingsEditor>> = const { RefCell::new(None) };
        static ON_APPLY: RefCell<Option<ApplyCallback>> = const { RefCell::new(None) };
        static RECORDING_KEY: Cell<Option<i32>> = const { Cell::new(None) };
        static MOUSE_ROW_COUNT: Cell<usize> = const { Cell::new(DEFAULT_MOUSE_BINDING_SLOTS) };
        static VIEWPORT_WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
        static CONTENT_WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
        static SYNCING: Cell<bool> = const { Cell::new(false) };
        static CURRENT_GROUP: Cell<usize> = const { Cell::new(NO_GROUP) };
        static GROUP_TEXTS: RefCell<[String; GROUP_COUNT]> = const {
            RefCell::new([
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ])
        };
        static CURRENT_PAGE: Cell<usize> = const { Cell::new(GROUP_GENERAL) };
        static REGISTRY: RefCell<Vec<RegEntry>> = const { RefCell::new(Vec::new()) };
    }

    fn mouse_row_count() -> usize {
        MOUSE_ROW_COUNT.with(Cell::get)
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn register_class() -> Result<(), String> {
        static STATE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let failure = STATE.get_or_init(|| {
            let class_name = wide(CLASS_NAME);
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: null_mut(),
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
            };
            let atom = unsafe { RegisterClassW(&class) };
            (atom == 0).then(|| "RegisterClassW failed for the settings window".to_string())
        });
        failure.clone().map_or(Ok(()), Err)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn create_child(
        hwnd: HWND,
        class: &str,
        text: &str,
        style: u32,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> HWND {
        unsafe {
            let child = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                w,
                h,
                hwnd,
                id as _,
                null_mut(),
                null_mut(),
            );
            if !child.is_null() {
                // Tag content children with their search group and collect
                // their visible words so the filter matches what the owner
                // sees. Creation-time x/y/h feed the filter snapshot, so no
                // coordinate mapping is needed later. Footer children are
                // built under NO_GROUP and skipped.
                let group = CURRENT_GROUP.with(Cell::get);
                SetWindowLongPtrW(child, GWLP_USERDATA, group as isize);
                if group != NO_GROUP {
                    if (class == "STATIC" || class == "BUTTON") && !text.is_empty() {
                        GROUP_TEXTS.with(|texts| {
                            let mut texts = texts.borrow_mut();
                            texts[group].push_str(&text.to_ascii_lowercase());
                            texts[group].push(' ');
                        });
                    }
                    REGISTRY.with(|registry| {
                        registry.borrow_mut().push(RegEntry {
                            hwnd: child,
                            group,
                            x,
                            y,
                            h,
                        });
                    });
                }
            }
            child
        }
    }

    /// One labelled row: a static label plus an edit control.
    unsafe fn add_row(hwnd: HWND, id: i32, label: &str, y: i32) {
        unsafe {
            create_child(hwnd, "STATIC", label, 0, id + 500, 12, y + 3, 155, 20);
            create_child(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER,
                id,
                175,
                y,
                130,
                24,
            );
            if let Some(hint) = setting_hint(label) {
                let help_id = id + 1500;
                let short = setting_short(label).unwrap_or(hint);
                create_child(hwnd, "STATIC", short, 0, help_id, 320, y, 350, 18);
                create_child(hwnd, "STATIC", hint, 0, id + 2500, 320, y + 17, 350, 42);
            }
        }
    }

    /// Add row with a closed dropdown instead of a free-text edit. Same label
    /// and help-control ids as `add_row` so search and help stay uniform.
    unsafe fn add_layout_row(hwnd: HWND, id: i32, label: &str, y: i32) {
        unsafe {
            create_child(hwnd, "STATIC", label, 0, id + 500, 12, y + 3, 155, 20);
            let combo = create_child(
                hwnd,
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                id,
                175,
                y,
                130,
                140,
            );
            for choice in LAYOUT_LABELS {
                SendMessageW(combo, CB_ADDSTRING, 0, wide(choice).as_ptr() as LPARAM);
            }
            if let Some(hint) = setting_hint(label) {
                let help_id = id + 1500;
                let short = setting_short(label).unwrap_or(hint);
                create_child(hwnd, "STATIC", short, 0, help_id, 320, y, 350, 18);
                create_child(hwnd, "STATIC", hint, 0, id + 2500, 320, y + 17, 350, 42);
            }
        }
    }

    fn setting_hint(label: &str) -> Option<&'static str> {
        Some(match label {
            "Leader key" => {
                "What it does: hold CapsLock to keep the grid open; tap Left Shift to open it once."
            }
            "Hold (ms)" => "What it does: delay before pointer mode starts. Example: 200 ms.",
            "Start speed (px/s)" => "What it does: starting movement speed. Example: 300 px/s.",
            "Max speed (px/s)" => "What it does: fastest movement speed. Must be >= start speed.",
            "Ramp (ms)" => "What it does: acceleration time. Example: 500 ms feels smooth.",
            "Scroll step" => "What it does: scroll distance per key press. Example: 1 notch.",
            "Layout (dense|simple)" => {
                "What it does: choose grid style. Example: dense uses two letters."
            }
            "Nudge step (px)" => "What it does: fine movement after selection. Example: 1 px.",
            "Panel color (RRGGBB)" => "What it does: grid label background. Example: 181C26.",
            "Panel opacity (0-255)" => "What it does: label background strength. 255 is solid.",
            "Border color" => "What it does: grid line color. Example: 647A96.",
            "Border width" => "What it does: grid line thickness. Example: 1 px.",
            "Highlight color" => "What it does: selected cell color. Example: FFC440.",
            "Highlight opacity" => "What it does: selected cell strength. 255 is solid.",
            "Label color" => "What it does: grid letter color. Use a bright contrast color.",
            "Pointer color" => "What it does: target marker color. Example: FF6060.",
            "Label size (1-8)" => "What it does: grid letter size. Example: 3 is the default.",
            "Nested rows" => "What it does: subgrid height. Example: 3 rows.",
            "Nested columns" => "What it does: subgrid width. Example: 10 columns.",
            "Nested keys (space separated)" => "What it does: final target keys. Example: Q W E R.",
            "Outer columns (space separated)" => {
                "What it does: first grid-letter bank. Example: A S D."
            }
            "Outer rows (space separated)" => {
                "What it does: second grid-letter bank. Example: Q W E."
            }
            _ => return None,
        })
    }

    fn setting_short(label: &str) -> Option<&'static str> {
        let id = match label {
            "Leader key" => "leader",
            "Hold (ms)" => "hold_ms",
            "Start speed (px/s)" => "start_speed",
            "Max speed (px/s)" => "max_speed",
            "Ramp (ms)" => "ramp_ms",
            "Scroll step" => "scroll_step",
            "Layout (dense|simple)" => "layout",
            "Nudge step (px)" => "nudge_step",
            "Panel color (RRGGBB)" => "color_panel",
            "Panel opacity (0-255)" => "opacity",
            "Border color" => "color_border",
            "Highlight color" => "color_highlight",
            "Label color" => "color_label",
            "Pointer color" => "color_pointer",
            "Label size (1-8)" => "opacity",
            "Nested rows" | "Nested columns" => "nested_size",
            "Nested keys (space separated)" => "nested_keys",
            "Outer columns (space separated)" => "column_keys",
            "Outer rows (space separated)" => "row_keys",
            _ => return None,
        };
        crate::settings_help::for_id(id).map(|help| help.short)
    }

    fn detail_for_control(id: i32) -> Option<&'static str> {
        #[cfg(test)]
        let key = ALL_CONTROL_IDS
            .iter()
            .find(|(_, control)| *control == id)
            .map(|(key, _)| *key)?;
        #[cfg(not(test))]
        let key = match id {
            ID_LEADER => "leader",
            ID_HOLD_MS => "hold_ms",
            ID_START_SPEED => "start_speed",
            ID_MAX_SPEED => "max_speed",
            ID_RAMP => "ramp_ms",
            ID_SCROLL_STEP => "scroll_step",
            ID_LAYOUT => "layout",
            ID_NUDGE_STEP => "nudge_step",
            ID_PANEL => "color_panel",
            ID_PANEL_OPACITY | ID_LABEL_SIZE => "opacity",
            ID_BORDER => "color_border",
            ID_HIGHLIGHT => "color_highlight",
            ID_LABEL_COLOR => "color_label",
            ID_POINTER => "color_pointer",
            ID_GRID_KEYS => "nested_keys",
            ID_COLUMN_KEYS => "column_keys",
            ID_ROW_KEYS => "row_keys",
            _ => return None,
        };
        crate::settings_help::for_id(key).map(|help| help.detail)
    }

    #[cfg(test)]
    fn help_control_id(id: &str) -> Option<i32> {
        ALL_CONTROL_IDS
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, control)| control + 1500)
    }

    #[cfg(test)]
    fn input_control_id(id: &str) -> Option<i32> {
        ALL_CONTROL_IDS
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, control)| *control)
    }

    #[cfg(test)]
    const ALL_CONTROL_IDS: &[(&str, i32)] = &[
        ("enabled", ID_ENABLED),
        ("mouse_bindings", MOUSE_KEY_BASE),
        ("leader", ID_LEADER),
        ("hold_ms", ID_HOLD_MS),
        ("start_speed", ID_START_SPEED),
        ("max_speed", ID_MAX_SPEED),
        ("ramp_ms", ID_RAMP),
        ("scroll_step", ID_SCROLL_STEP),
        ("layout", ID_LAYOUT),
        ("nudge_step", ID_NUDGE_STEP),
        ("color_panel", ID_PANEL),
        ("opacity", ID_PANEL_OPACITY),
        ("color_border", ID_BORDER),
        ("color_highlight", ID_HIGHLIGHT),
        ("color_label", ID_LABEL_COLOR),
        ("color_pointer", ID_POINTER),
        ("nested_size", ID_GRID_ROWS),
        ("nested_keys", ID_GRID_KEYS),
        ("column_keys", ID_COLUMN_KEYS),
        ("row_keys", ID_ROW_KEYS),
        ("auto_free", ID_AUTO_FREE),
        ("nudge_enabled", ID_NUDGE_ENABLE),
        ("drag_after_select", ID_DRAG),
    ];

    unsafe fn add_mouse_binding_row(hwnd: HWND, index: usize, y: i32) {
        unsafe {
            let index = index as i32;
            create_child(
                hwnd,
                "EDIT",
                "",
                WS_TABSTOP | WS_BORDER,
                MOUSE_KEY_BASE + index,
                12,
                y,
                72,
                24,
            );
            let combo = create_child(
                hwnd,
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                MOUSE_ACTION_BASE + index,
                90,
                y,
                125,
                24,
            );
            for label in ACTION_LABELS {
                SendMessageW(combo, CB_ADDSTRING, 0, wide(label).as_ptr() as LPARAM);
            }
            create_child(
                hwnd,
                "BUTTON",
                "Record",
                WS_TABSTOP,
                MOUSE_RECORD_BASE + index,
                222,
                y,
                62,
                24,
            );
            create_child(
                hwnd,
                "BUTTON",
                "Clear",
                WS_TABSTOP,
                MOUSE_CLEAR_BASE + index,
                290,
                y,
                56,
                24,
            );
        }
    }

    unsafe fn build_controls(content: HWND, hwnd: HWND, footer_y: i32) -> i32 {
        unsafe {
            let mut y = 12;
            // Fresh snapshot per window: thread-locals survive across test
            // windows on one thread, and stale handles must never filter.
            REGISTRY.with(|registry| registry.borrow_mut().clear());
            GROUP_TEXTS.with(|texts| {
                *texts.borrow_mut() = [
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ];
            });
            CURRENT_PAGE.with(|page| page.set(GROUP_GENERAL));
            // --- General page ---
            CURRENT_GROUP.with(|group| group.set(GROUP_GENERAL));
            create_child(
                content,
                "STATIC",
                "General",
                0,
                ID_MOTION_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            create_child(
                content,
                "BUTTON",
                "Enabled",
                CHECKBOX_STYLE,
                ID_ENABLED,
                12,
                y,
                155,
                20,
            );
            y += 72;
            add_row(content, ID_LEADER, "Leader key", y);
            y += 72;
            add_row(content, ID_HOLD_MS, "Hold (ms)", y);
            y += 72;
            create_child(
                content,
                "STATIC",
                "Built-in shortcuts: tap Left Shift to open grid; tap Left Ctrl to toggle free mode.",
                0,
                ID_MOTION_LABEL + 2500,
                12,
                y,
                510,
                36,
            );
            y += 48;
            // --- Movement page ---
            CURRENT_GROUP.with(|group| group.set(GROUP_MOVEMENT));
            create_child(
                content,
                "STATIC",
                "Movement",
                0,
                ID_MOVEMENT_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            add_row(content, ID_START_SPEED, "Start speed (px/s)", y);
            y += 72;
            add_row(content, ID_MAX_SPEED, "Max speed (px/s)", y);
            y += 72;
            add_row(content, ID_RAMP, "Ramp (ms)", y);
            y += 72;
            add_row(content, ID_SCROLL_STEP, "Scroll step", y);
            y += 72;
            create_child(
                content,
                "BUTTON",
                "Reset settings",
                WS_TABSTOP,
                ID_RESET_SETTINGS,
                12,
                y,
                130,
                24,
            );
            y += 30;
            // --- Grid page ---
            CURRENT_GROUP.with(|group| group.set(GROUP_GRID));
            create_child(
                content,
                "STATIC",
                "Grid geometry and keys",
                0,
                ID_GRID_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            add_layout_row(content, ID_LAYOUT, "Layout (dense|simple)", y);
            y += 72;
            create_child(
                content,
                "BUTTON",
                "Nudge enabled",
                CHECKBOX_STYLE,
                ID_NUDGE_ENABLE,
                12,
                y,
                155,
                20,
            );
            y += 72;
            add_row(content, ID_NUDGE_STEP, "Nudge step (px)", y);
            y += 72;
            create_child(
                content,
                "BUTTON",
                "Drag after selection",
                CHECKBOX_STYLE,
                ID_DRAG,
                12,
                y,
                170,
                20,
            );
            y += 30;
            create_child(
                content,
                "BUTTON",
                "Free mode after move",
                CHECKBOX_STYLE,
                ID_AUTO_FREE,
                12,
                y,
                170,
                20,
            );
            y += 34;
            CURRENT_GROUP.with(|group| group.set(GROUP_APPEARANCE));
            create_child(
                content,
                "STATIC",
                "Appearance",
                0,
                ID_APPEAR_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            add_row(content, ID_PANEL, "Panel color (RRGGBB)", y);
            y += 72;
            add_row(content, ID_PANEL_OPACITY, "Panel opacity (0-255)", y);
            y += 72;
            add_row(content, ID_BORDER, "Border color", y);
            y += 72;
            add_row(content, ID_BORDER_PX, "Border width", y);
            y += 72;
            add_row(content, ID_HIGHLIGHT, "Highlight color", y);
            y += 72;
            add_row(content, ID_HIGHLIGHT_OPACITY, "Highlight opacity", y);
            y += 72;
            add_row(content, ID_LABEL_COLOR, "Label color", y);
            y += 72;
            add_row(content, ID_POINTER, "Pointer color", y);
            y += 72;
            add_row(content, ID_LABEL_SIZE, "Label size (1-8)", y);
            y += 72;
            create_child(
                content,
                "BUTTON",
                "Reset appearance",
                WS_TABSTOP,
                ID_RESET_APPEARANCE,
                12,
                y,
                140,
                24,
            );
            y += 36;

            CURRENT_GROUP.with(|group| group.set(GROUP_SHORTCUTS));
            GROUP_TEXTS.with(|texts| {
                let mut texts = texts.borrow_mut();
                texts[GROUP_SHORTCUTS].push_str(&ACTIONS.join(" "));
                texts[GROUP_SHORTCUTS].push(' ');
                texts[GROUP_SHORTCUTS].push_str(&ACTION_LABELS.join(" "));
            });

            create_child(
                content,
                "STATIC",
                "Mouse bindings",
                0,
                ID_MOUSE_BINDINGS_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            create_child(content, "STATIC", "Key", 0, ID_KEY_LABEL, 12, y, 72, 20);
            create_child(
                content,
                "STATIC",
                "Action",
                0,
                ID_ACTION_LABEL,
                90,
                y,
                125,
                20,
            );
            y += 20;
            for index in 0..mouse_row_count() {
                add_mouse_binding_row(content, index, y);
                y += 30;
            }
            create_child(content, "STATIC", "", 0, ID_BINDING_ERROR, 12, y, 350, 32);
            y += 32;
            create_child(
                content,
                "BUTTON",
                "Reset bindings",
                WS_TABSTOP,
                ID_RESET_BINDINGS,
                12,
                y,
                130,
                24,
            );
            y += 30;

            CURRENT_GROUP.with(|group| group.set(GROUP_GRID));

            add_row(content, ID_GRID_ROWS, "Nested rows", y);
            y += 72;
            add_row(content, ID_GRID_COLS, "Nested columns", y);
            y += 72;
            add_row(content, ID_GRID_KEYS, "Nested keys (space separated)", y);
            y += 72;
            add_row(
                content,
                ID_COLUMN_KEYS,
                "Outer columns (space separated)",
                y,
            );
            y += 72;
            add_row(content, ID_ROW_KEYS, "Outer rows (space separated)", y);
            y += 72;
            create_child(content, "STATIC", "", 0, ID_GRID_ERROR, 12, y, 350, 32);
            y += 32;
            create_child(
                content,
                "BUTTON",
                "Reset grid",
                WS_TABSTOP,
                ID_RESET_GRID,
                12,
                y,
                130,
                24,
            );
            y += 30;
            create_child(
                content,
                "STATIC",
                // Dead note: initial-layer bindings have a consumer now; the
                // control stays for layout but renders no text.
                "",
                0,
                ID_INITIAL_BINDINGS_NOTE,
                12,
                y,
                360,
                24,
            );
            y += 30;
            // --- About page ---
            CURRENT_GROUP.with(|group| group.set(GROUP_ABOUT));
            create_child(
                content,
                "STATIC",
                "About",
                0,
                ID_ABOUT_LABEL,
                12,
                y,
                300,
                22,
            );
            y += 24;
            create_child(
                content,
                "STATIC",
                &format!("Clickless {}", env!("CARGO_PKG_VERSION")),
                0,
                ID_VERSION_TEXT,
                12,
                y,
                300,
                20,
            );
            y += 28;
            create_child(
                content,
                "BUTTON",
                "Reset all",
                WS_TABSTOP,
                ID_RESET_ALL,
                12,
                y,
                130,
                24,
            );
            y += 30;
            create_child(
                content,
                "BUTTON",
                "Copy diagnostics",
                WS_TABSTOP,
                ID_COPY_DIAG,
                12,
                y,
                140,
                24,
            );
            create_child(
                content,
                "BUTTON",
                "Open log folder",
                WS_TABSTOP,
                ID_OPEN_LOG,
                160,
                y,
                130,
                24,
            );
            y += 30;
            create_child(
                content,
                "BUTTON",
                "Practice again",
                WS_TABSTOP,
                ID_PRACTICE_AGAIN,
                12,
                y,
                130,
                24,
            );
            y += 32;

            // Footer lives on the top-level window: never tagged, never
            // filtered, never moved by search.
            CURRENT_GROUP.with(|group| group.set(NO_GROUP));
            create_child(hwnd, "STATIC", "", 0, ID_MESSAGE, 12, footer_y + 8, 270, 40);
            create_child(
                hwnd,
                "BUTTON",
                "Apply",
                WS_TABSTOP,
                ID_APPLY,
                300,
                footer_y + 4,
                80,
                26,
            );
            create_child(
                hwnd,
                "BUTTON",
                "Save",
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                ID_SAVE,
                300,
                footer_y + 34,
                80,
                26,
            );
            create_child(
                hwnd,
                "BUTTON",
                "Cancel",
                WS_TABSTOP,
                ID_CANCEL,
                300,
                footer_y + 64,
                80,
                26,
            );
            create_child(
                hwnd,
                "BUTTON",
                "Reset page",
                WS_TABSTOP,
                ID_RESET_PAGE,
                300,
                footer_y + 94,
                80,
                26,
            );
            y
        }
    }

    /// Which groups match a search query. Empty queries match nothing here;
    /// callers restore the current page instead.
    fn groups_matching(query: &str) -> [bool; GROUP_COUNT] {
        let q = query.trim().to_ascii_lowercase();
        let mut visible = [false; GROUP_COUNT];
        GROUP_TEXTS.with(|texts| {
            let texts = texts.borrow();
            for (group, shown) in visible.iter_mut().enumerate() {
                *shown = !q.is_empty() && texts[group].contains(&q);
            }
        });
        visible
    }

    /// Shows exactly the flagged groups, compacted to the top from the
    /// creation snapshot. Hidden controls leave tab order automatically
    /// (IsDialogMessageW skips invisible windows); footer stays fixed.
    unsafe fn apply_groups(top: HWND, visible: [bool; GROUP_COUNT]) {
        unsafe {
            let entries: Vec<RegEntry> = REGISTRY.with(|registry| registry.borrow().clone());
            let mut cursor = 12;
            for (group, shown) in visible.iter().enumerate() {
                let first = entries
                    .iter()
                    .filter(|entry| entry.group == group)
                    .map(|entry| entry.y)
                    .min();
                let last = entries
                    .iter()
                    .filter(|entry| entry.group == group)
                    .map(|entry| entry.y + entry.h)
                    .max();
                let (Some(first), Some(last)) = (first, last) else {
                    continue;
                };
                if !shown {
                    for entry in entries.iter().filter(|entry| entry.group == group) {
                        ShowWindow(entry.hwnd, SW_HIDE);
                    }
                    continue;
                }
                let delta = cursor - first;
                for entry in entries.iter().filter(|entry| entry.group == group) {
                    ShowWindow(entry.hwnd, SW_SHOW);
                    if delta != 0 {
                        SetWindowPos(
                            entry.hwnd,
                            null_mut(),
                            entry.x,
                            entry.y + delta,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS,
                        );
                    }
                }
                cursor += last - first + 8;
            }
            let viewport = VIEWPORT_WINDOW.with(Cell::get);
            let mut area = RECT::default();
            GetClientRect(viewport, &mut area);
            set_scroll_content(top, cursor + 12, area.bottom);
            scroll_to(top, 0);
            // Synchronous repaint like scroll_to: hiding, showing and moving
            // native children must finish before the next update lands.
            RedrawWindow(
                viewport,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }

    /// Shows one sidebar page. Clearing the search box restores this page,
    /// never the whole form.
    unsafe fn show_page(top: HWND, page: usize) {
        unsafe {
            CURRENT_PAGE.with(|current| current.set(page));
            let mut visible = [false; GROUP_COUNT];
            if page < GROUP_COUNT {
                visible[page] = true;
            }
            apply_groups(top, visible);
        }
    }

    /// Live search: a query stacks every matching group; clearing the box
    /// restores the current sidebar page.
    unsafe fn apply_search(top: HWND, query: &str) {
        unsafe {
            if query.trim().is_empty() {
                let page = CURRENT_PAGE.with(Cell::get);
                show_page(top, page);
            } else {
                apply_groups(top, groups_matching(query));
            }
        }
    }

    unsafe fn set_scroll_content(hwnd: HWND, content_height: i32, viewport_height: i32) {
        unsafe {
            let info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: content_height.saturating_sub(1),
                nPage: viewport_height.max(0) as u32,
                nPos: 0,
                nTrackPos: 0,
            };
            SetScrollInfo(hwnd, SB_VERT, &info, 1);
        }
    }

    /// Sidebar width and search-strip height. The viewport starts below the
    /// search row and right of the sidebar; the footer spans the full width.
    const SIDEBAR_WIDTH: i32 = 150;
    const TOP_STRIP_HEIGHT: i32 = 40;

    unsafe fn create_scroll_surfaces(hwnd: HWND) -> (HWND, HWND, i32, i32) {
        unsafe {
            let mut client = RECT::default();
            GetClientRect(hwnd, &mut client);
            let viewport_height = client.bottom - TOP_STRIP_HEIGHT - FOOTER_HEIGHT;
            let content_width = (client.right - SIDEBAR_WIDTH).max(200);
            let viewport = create_container(
                hwnd,
                701,
                SIDEBAR_WIDTH,
                TOP_STRIP_HEIGHT,
                content_width,
                viewport_height,
            );
            let content = create_container(viewport, 702, 0, 0, content_width, 2048);
            VIEWPORT_WINDOW.with(|cell| cell.set(viewport));
            CONTENT_WINDOW.with(|cell| cell.set(content));
            (viewport, content, viewport_height, content_width)
        }
    }

    unsafe fn create_container(
        parent: HWND,
        id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> HWND {
        unsafe {
            CreateWindowExW(
                WS_EX_CONTROLPARENT,
                wide(CLASS_NAME).as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                x,
                y,
                width,
                height,
                parent,
                id as _,
                null_mut(),
                null_mut(),
            )
        }
    }

    unsafe fn scroll_to(hwnd: HWND, requested: i32) {
        unsafe {
            let mut info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_ALL,
                ..Default::default()
            };
            if GetScrollInfo(hwnd, SB_VERT, &mut info) == 0 {
                return;
            }
            let max = info
                .nMax
                .saturating_sub(info.nPage as i32)
                .saturating_add(1);
            let next = requested.clamp(info.nMin, max.max(info.nMin));
            if next == info.nPos {
                return;
            }
            info.fMask = SIF_POS;
            info.nPos = next;
            SetScrollInfo(hwnd, SB_VERT, &info, 1);
            // Reposition content window; viewport has WS_CLIPCHILDREN to clip
            let content = CONTENT_WINDOW.with(Cell::get);
            if !content.is_null() {
                SetWindowPos(
                    content,
                    null_mut(),
                    0,
                    -next,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS,
                );
                // Invalidate only the viewport, no RDW_ERASE to prevent flicker
                let viewport = VIEWPORT_WINDOW.with(Cell::get);
                if !viewport.is_null() {
                    RedrawWindow(
                        viewport,
                        null(),
                        null_mut(),
                        RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
                    );
                }
            }
        }
    }

    unsafe fn scroll_command(hwnd: HWND, command: i32) {
        unsafe {
            let mut info = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_ALL,
                ..Default::default()
            };
            if GetScrollInfo(hwnd, SB_VERT, &mut info) == 0 {
                return;
            }
            let page = (info.nPage as i32 - 28).max(28);
            let next = match command {
                SB_LINEUP => info.nPos - 28,
                SB_LINEDOWN => info.nPos + 28,
                SB_PAGEUP => info.nPos - page,
                SB_PAGEDOWN => info.nPos + page,
                SB_THUMBPOSITION | SB_THUMBTRACK => info.nTrackPos,
                SB_TOP => info.nMin,
                SB_BOTTOM => info.nMax,
                _ => return,
            };
            scroll_to(hwnd, next);
        }
    }

    unsafe fn set_check(hwnd: HWND, id: i32, checked: bool) {
        unsafe {
            SendMessageW(
                get_control(hwnd, id),
                BM_SETCHECK,
                if checked { BST_CHECKED } else { BST_UNCHECKED },
                0,
            );
        }
    }

    unsafe fn get_check(hwnd: HWND, id: i32) -> bool {
        unsafe { SendMessageW(get_control(hwnd, id), BM_GETCHECK, 0, 0) as usize == BST_CHECKED }
    }

    fn get_control(hwnd: HWND, id: i32) -> HWND {
        unsafe {
            let direct = GetDlgItem(hwnd, id);
            if !direct.is_null() {
                return direct;
            }
            let viewport = VIEWPORT_WINDOW.with(Cell::get);
            let nested = GetDlgItem(viewport, id);
            if !nested.is_null() {
                return nested;
            }
            let content = CONTENT_WINDOW.with(Cell::get);
            let nested = GetDlgItem(content, id);
            if !nested.is_null() {
                return nested;
            }
            let mut parent = GetParent(hwnd);
            while !parent.is_null() {
                let nested = GetDlgItem(parent, id);
                if !nested.is_null() {
                    return nested;
                }
                parent = GetParent(parent);
            }
            null_mut()
        }
    }

    unsafe fn set_text(hwnd: HWND, id: i32, text: &str) {
        unsafe { SetWindowTextW(get_control(hwnd, id), wide(text).as_ptr()) };
    }

    unsafe fn get_text(hwnd: HWND, id: i32) -> String {
        unsafe {
            let control = get_control(hwnd, id);
            if control.is_null() {
                return String::new();
            }
            let len = GetWindowTextLengthW(control);
            if len <= 0 {
                return String::new();
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            GetWindowTextW(control, buf.as_mut_ptr(), len + 1);
            String::from_utf16_lossy(&buf[..len as usize])
        }
    }

    unsafe fn set_combo(hwnd: HWND, id: i32, value: &str) {
        unsafe {
            let combo = get_control(hwnd, id);
            let value = wide(display_label(value));
            let index = SendMessageW(
                combo,
                CB_FINDSTRINGEXACT,
                usize::MAX,
                value.as_ptr() as LPARAM,
            );
            SendMessageW(
                combo,
                CB_SETCURSEL,
                if index < 0 {
                    usize::MAX
                } else {
                    index as usize
                },
                0,
            );
        }
    }

    unsafe fn get_combo(hwnd: HWND, id: i32) -> String {
        unsafe {
            let combo = get_control(hwnd, id);
            let index = SendMessageW(combo, CB_GETCURSEL, 0, 0);
            if index < 0 {
                return String::new();
            }
            let length = SendMessageW(combo, CB_GETLBTEXTLEN, index as usize, 0);
            if length < 0 {
                return String::new();
            }
            let mut text = vec![0u16; length as usize + 1];
            SendMessageW(
                combo,
                CB_GETLBTEXT,
                index as usize,
                text.as_mut_ptr() as LPARAM,
            );
            stored_token(&String::from_utf16_lossy(&text[..length as usize])).to_string()
        }
    }

    fn parse_key_list(text: String) -> Vec<String> {
        // Whitespace-separated only: Comma's own "," label must survive as
        // a token, so commas can never be separators here. The parser still
        // accepts the "comma", "dot" and "space" words for manual entry.
        text.split_whitespace().map(str::to_string).collect()
    }

    unsafe fn load_fields(hwnd: HWND, config: &clickless_config::Config) {
        unsafe {
            let fields = fields_from_config(config);
            set_check(hwnd, ID_ENABLED, fields.enabled);
            set_text(hwnd, ID_LEADER, &fields.leader);
            set_text(hwnd, ID_START_SPEED, &fields.start_speed_px_s);
            set_text(hwnd, ID_MAX_SPEED, &fields.max_speed_px_s);
            set_text(hwnd, ID_RAMP, &fields.ramp_ms);
            set_text(hwnd, ID_HOLD_MS, &fields.hold_ms);
            set_text(hwnd, ID_SCROLL_STEP, &fields.scroll_step);
            set_combo(hwnd, ID_LAYOUT, &fields.layout);
            for index in 0..mouse_row_count() {
                let (key, action) = fields
                    .mouse_bindings
                    .get(index)
                    .map(|(key, action)| (key.as_str(), action.as_str()))
                    .unwrap_or(("", ""));
                set_text(hwnd, MOUSE_KEY_BASE + index as i32, key);
                set_combo(hwnd, MOUSE_ACTION_BASE + index as i32, action);
            }
            set_text(hwnd, ID_GRID_ROWS, &fields.grid_rows);
            set_text(hwnd, ID_GRID_COLS, &fields.grid_cols);
            set_text(hwnd, ID_GRID_KEYS, &fields.grid_keys.join(" "));
            set_text(hwnd, ID_COLUMN_KEYS, &fields.column_keys.join(" "));
            set_text(hwnd, ID_ROW_KEYS, &fields.row_keys.join(" "));
            set_check(hwnd, ID_NUDGE_ENABLE, fields.nudge_enabled);
            set_text(hwnd, ID_NUDGE_STEP, &fields.nudge_step_px);
            set_check(hwnd, ID_DRAG, fields.drag_after_select);
            set_check(hwnd, ID_AUTO_FREE, fields.auto_free_mode);
            set_text(hwnd, ID_PANEL, &fields.panel);
            set_text(hwnd, ID_PANEL_OPACITY, &fields.panel_opacity);
            set_text(hwnd, ID_BORDER, &fields.border);
            set_text(hwnd, ID_BORDER_PX, &fields.border_px);
            set_text(hwnd, ID_HIGHLIGHT, &fields.highlight);
            set_text(hwnd, ID_HIGHLIGHT_OPACITY, &fields.highlight_opacity);
            set_text(hwnd, ID_LABEL_COLOR, &fields.label);
            set_text(hwnd, ID_POINTER, &fields.pointer);
            set_text(hwnd, ID_LABEL_SIZE, &fields.label_size);
            set_text(hwnd, ID_MESSAGE, "");
            set_text(hwnd, ID_BINDING_ERROR, "");
            set_text(hwnd, ID_GRID_ERROR, "");
        }
    }

    unsafe fn read_fields(hwnd: HWND) -> Fields {
        unsafe {
            Fields {
                enabled: get_check(hwnd, ID_ENABLED),
                leader: get_text(hwnd, ID_LEADER),
                start_speed_px_s: get_text(hwnd, ID_START_SPEED),
                max_speed_px_s: get_text(hwnd, ID_MAX_SPEED),
                ramp_ms: get_text(hwnd, ID_RAMP),
                hold_ms: get_text(hwnd, ID_HOLD_MS),
                scroll_step: get_text(hwnd, ID_SCROLL_STEP),
                layout: get_combo(hwnd, ID_LAYOUT),
                mouse_bindings: (0..mouse_row_count())
                    .map(|index| {
                        (
                            get_text(hwnd, MOUSE_KEY_BASE + index as i32),
                            get_combo(hwnd, MOUSE_ACTION_BASE + index as i32),
                        )
                    })
                    .collect(),
                grid_rows: get_text(hwnd, ID_GRID_ROWS),
                grid_cols: get_text(hwnd, ID_GRID_COLS),
                grid_keys: parse_key_list(get_text(hwnd, ID_GRID_KEYS)),
                column_keys: parse_key_list(get_text(hwnd, ID_COLUMN_KEYS)),
                row_keys: parse_key_list(get_text(hwnd, ID_ROW_KEYS)),
                nudge_enabled: get_check(hwnd, ID_NUDGE_ENABLE),
                nudge_step_px: get_text(hwnd, ID_NUDGE_STEP),
                drag_after_select: get_check(hwnd, ID_DRAG),
                auto_free_mode: get_check(hwnd, ID_AUTO_FREE),
                panel: get_text(hwnd, ID_PANEL),
                panel_opacity: get_text(hwnd, ID_PANEL_OPACITY),
                border: get_text(hwnd, ID_BORDER),
                highlight_opacity: get_text(hwnd, ID_HIGHLIGHT_OPACITY),
                border_px: get_text(hwnd, ID_BORDER_PX),
                highlight: get_text(hwnd, ID_HIGHLIGHT),
                label: get_text(hwnd, ID_LABEL_COLOR),
                pointer: get_text(hwnd, ID_POINTER),
                label_size: get_text(hwnd, ID_LABEL_SIZE),
            }
        }
    }

    unsafe fn handle_apply(hwnd: HWND, save: bool) {
        unsafe {
            let fields = read_fields(hwnd);
            let mut message = "editor not initialised".to_string();
            EDITOR.with(|cell| {
                if let Some(editor) = cell.borrow_mut().as_mut() {
                    message = match editor.edit(&fields) {
                        Err(e) => e,
                        Ok(()) => ON_APPLY.with(|apply_cell| {
                            let mut apply = apply_cell.borrow_mut();
                            let Some(callback) = apply.as_mut() else {
                                return "runtime is unavailable".to_string();
                            };
                            if save {
                                match save_target() {
                                    Err(e) => e,
                                    Ok(path) => editor
                                        .save_after_runtime_apply(&path, |config| callback(config))
                                        .map(|_| format!("Saved to {}", path.display()))
                                        .unwrap_or_else(|e| e),
                                }
                            } else {
                                editor
                                    .apply_to_runtime(|config| callback(config))
                                    .map(|_| "Applied.".to_string())
                                    .unwrap_or_else(|e| e)
                            }
                        }),
                    }
                }
            });
            route_message(hwnd, &message);
            // A validation error moves focus to the field it names so the
            // user lands exactly where the fix belongs. Success and
            // infrastructure messages match nothing and leave focus alone.
            if let Some(id) = invalid_control(&message) {
                SetFocus(get_control(hwnd, id));
            }
        }
    }

    /// Maps an editor error message to the control that owns the named
    /// field. Ordered: the max-speed message also contains "start speed",
    /// and opacity/size names must win over their shorter prefixes.
    fn invalid_control(message: &str) -> Option<i32> {
        const MAP: &[(&str, i32)] = &[
            ("max speed", ID_MAX_SPEED),
            ("start speed", ID_START_SPEED),
            ("ramp", ID_RAMP),
            ("hold", ID_HOLD_MS),
            ("scroll step", ID_SCROLL_STEP),
            ("leader", ID_LEADER),
            ("layout", ID_LAYOUT),
            ("nudge step", ID_NUDGE_STEP),
            ("panel opacity", ID_PANEL_OPACITY),
            ("panel_opacity", ID_PANEL_OPACITY),
            ("panel", ID_PANEL),
            ("border width", ID_BORDER_PX),
            ("border_px", ID_BORDER_PX),
            ("border", ID_BORDER),
            ("highlight opacity", ID_HIGHLIGHT_OPACITY),
            ("highlight_opacity", ID_HIGHLIGHT_OPACITY),
            ("highlight", ID_HIGHLIGHT),
            ("label size", ID_LABEL_SIZE),
            ("label_size", ID_LABEL_SIZE),
            ("label", ID_LABEL_COLOR),
            ("pointer", ID_POINTER),
            ("bindings:", MOUSE_KEY_BASE),
            ("grid:", ID_GRID_ROWS),
        ];
        MAP.iter()
            .find(|(name, _)| message.contains(name))
            .map(|(_, id)| *id)
    }

    /// Routes an editor message to the bindings, grid or general line.
    unsafe fn route_message(hwnd: HWND, message: &str) {
        unsafe {
            if let Some(error) = message.strip_prefix("bindings:") {
                set_text(hwnd, ID_BINDING_ERROR, error.trim());
                set_text(hwnd, ID_GRID_ERROR, "");
                set_text(hwnd, ID_MESSAGE, "");
            } else if let Some(error) = message.strip_prefix("grid:") {
                set_text(hwnd, ID_GRID_ERROR, error.trim());
                set_text(hwnd, ID_BINDING_ERROR, "");
                set_text(hwnd, ID_MESSAGE, "");
            } else {
                set_text(hwnd, ID_BINDING_ERROR, "");
                set_text(hwnd, ID_GRID_ERROR, "");
                set_text(hwnd, ID_MESSAGE, message);
            }
        }
    }

    /// Live draft sync: every value change reloads the draft so the footer
    /// dirty state is always honest. Errors route to the message lines
    /// immediately; Apply re-reports them if clicked while invalid.
    /// Reentrancy guard: routing writes control text, and writing text fires
    /// EN_CHANGE synchronously, which would sync again mid-borrow and
    /// recurse forever on a standing error.
    unsafe fn sync_draft(hwnd: HWND) {
        unsafe {
            if SYNCING.with(Cell::get) {
                return;
            }
            SYNCING.with(|syncing| syncing.set(true));
            let fields = read_fields(hwnd);
            let outcome = EDITOR.with(|cell| {
                cell.borrow_mut()
                    .as_mut()
                    .map(|editor| editor.edit(&fields))
            });
            if let Some(Err(message)) = outcome {
                route_message(hwnd, &message);
            }
            SYNCING.with(|syncing| syncing.set(false));
        }
    }

    /// Copies version, config summary and recent log lines to the clipboard.
    /// Never carries the full config, keys, typed text or pointer history:
    /// the builder below only accepts a pre-made summary string.
    unsafe fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{
            GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
        };
        // CF_UNICODETEXT from Winuser.h; avoids a second system feature.
        const CF_UNICODETEXT: u32 = 13;
        unsafe {
            if OpenClipboard(null_mut()) == 0 {
                return Err("clipboard is busy".to_string());
            }
            let result = (|| {
                if EmptyClipboard() == 0 {
                    return Err("failed to clear clipboard".to_string());
                }
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = wide.len() * 2;
                let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                if mem.is_null() {
                    return Err("clipboard allocation failed".to_string());
                }
                let lock = GlobalLock(mem);
                if lock.is_null() {
                    return Err("clipboard lock failed".to_string());
                }
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, lock as *mut u8, bytes);
                GlobalUnlock(mem);
                if SetClipboardData(CF_UNICODETEXT, mem).is_null() {
                    return Err("clipboard paste failed".to_string());
                }
                Ok(())
            })();
            CloseClipboard();
            result
        }
    }

    /// Builds diagnostics and either copies them or opens the log folder.
    /// The message line reports what happened; nothing here touches capture.
    unsafe fn handle_diagnostics(hwnd: HWND, copy: bool) {
        unsafe {
            let os = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
            let tail = crate::gui_error::log_file_path()
                .and_then(|path| std::fs::read(&path).ok())
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            let summary = match clickless_config::default_config_path() {
                Err(e) => format!("config path unavailable: {e}"),
                Ok(path) if !path.exists() => "no config file, using defaults".to_string(),
                Ok(path) => match clickless_config::Config::load_from_file(&path) {
                    Err(e) => format!("unparsable: {e}"),
                    Ok(config) => match config.validate() {
                        Ok(()) => "valid".to_string(),
                        Err(e) => format!("invalid: {e}"),
                    },
                },
            };
            // Note: a missing file and an unreadable file both mean defaults;
            // only the summary wording differs, never the content.
            let text =
                crate::gui_error::diagnostics_text(env!("CARGO_PKG_VERSION"), &os, &tail, &summary);
            let message = if copy {
                match copy_text_to_clipboard(&text) {
                    Ok(()) => "Diagnostics copied.".to_string(),
                    Err(e) => e,
                }
            } else {
                match crate::gui_error::log_file_path()
                    .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
                {
                    Some(dir) => match std::process::Command::new("explorer").arg(&dir).spawn() {
                        Ok(_) => format!("Opened {}.", dir.display()),
                        Err(e) => format!("cannot open log folder: {e}"),
                    },
                    None => "log folder unavailable".to_string(),
                }
            };
            set_text(hwnd, ID_MESSAGE, &message);
        }
    }

    fn save_target() -> Result<std::path::PathBuf, String> {
        clickless_config::default_config_path().map_err(|e| e.to_string())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Placement {
        x: i32,
        y: i32,
    }

    /// Parses one "x y" line. Anything else means centered defaults.
    fn parse_placement(text: &str) -> Option<Placement> {
        let mut parts = text.split_whitespace();
        let placement = Placement {
            x: parts.next()?.parse().ok()?,
            y: parts.next()?.parse().ok()?,
        };
        parts.next().is_none().then_some(placement)
    }

    fn render_placement(placement: Placement) -> String {
        format!("{} {}\n", placement.x, placement.y)
    }

    /// Centered 820x640, or top-left when the screen is smaller.
    fn default_placement(screen_w: i32, screen_h: i32) -> Placement {
        Placement {
            x: (screen_w - SETTINGS_WIDTH).max(0) / 2,
            y: (screen_h - SETTINGS_HEIGHT).max(0) / 2,
        }
    }

    /// Keeps a 100px strip of the fixed-size window on the primary screen
    /// so a restored position is never lost off-screen (unplugged monitor).
    fn clamp_placement(placement: Placement, screen_w: i32, screen_h: i32) -> Placement {
        Placement {
            x: placement.x.clamp(
                100 - SETTINGS_WIDTH,
                (screen_w - 100).max(100 - SETTINGS_WIDTH),
            ),
            y: placement.y.clamp(0, (screen_h - 100).max(0)),
        }
    }

    fn placement_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(
            std::path::Path::new(&base)
                .join("clickless")
                .join("window-position.txt"),
        )
    }

    fn load_placement(screen_w: i32, screen_h: i32) -> Placement {
        let saved = placement_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| parse_placement(&text));
        match saved {
            Some(placement) => clamp_placement(placement, screen_w, screen_h),
            None => default_placement(screen_w, screen_h),
        }
    }

    /// Persists the top-left corner. Best effort: a failing write keeps the
    /// previous position file, never an error surface.
    unsafe fn save_placement(hwnd: HWND) {
        unsafe {
            let Some(path) = placement_path() else {
                return;
            };
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                &path,
                render_placement(Placement {
                    x: rect.left,
                    y: rect.top,
                }),
            );
        }
    }

    /// UI font: Segoe UI Variable with Segoe UI fallback, 9pt at the
    /// window's DPI. Returns the face that actually materialized.
    unsafe fn create_ui_font(dpi: u32) -> (HFONT, String) {
        unsafe {
            let height = -((9 * dpi as i32) / 72).max(1);
            for face in ["Segoe UI Variable Text", "Segoe UI"] {
                let wide_face = wide(face);
                let font = CreateFontW(
                    height,
                    0,
                    0,
                    0,
                    FW_NORMAL as i32,
                    0u32,
                    0u32,
                    0u32,
                    DEFAULT_CHARSET as u32,
                    OUT_TT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    DEFAULT_QUALITY as u32,
                    FF_DONTCARE as u32,
                    wide_face.as_ptr() as windows_sys::core::PCWSTR,
                );
                if font.is_null() {
                    continue;
                }
                let dc = GetDC(null_mut());
                let selected = SelectObject(dc, font);
                let mut name = vec![0u16; 64];
                let len = GetTextFaceW(dc, name.len() as i32, name.as_mut_ptr());
                SelectObject(dc, selected);
                ReleaseDC(null_mut(), dc);
                // Length counts the NUL terminator: strip it before compare.
                let actual =
                    String::from_utf16_lossy(&name[..len.max(0) as usize]).replace('\0', "");
                if actual.eq_ignore_ascii_case(face) {
                    return (font, actual);
                }
                DeleteObject(font);
            }
            (null_mut(), String::new())
        }
    }

    unsafe extern "system" fn apply_font_child(hwnd: HWND, lparam: LPARAM) -> i32 {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, lparam as usize, 1);
            EnumChildWindows(hwnd, Some(apply_font_child), lparam);
            1
        }
    }

    /// Footer states: Apply/Save follow the editor dirty flag (Save stays
    /// the primary button); Reset page is dead on About, which owns the
    /// global Reset-all instead.
    unsafe fn refresh_footer(hwnd: HWND) {
        unsafe {
            let dirty = EDITOR.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|editor| editor.is_dirty())
            });
            let page = CURRENT_PAGE.with(Cell::get);
            for (id, on) in [
                (ID_APPLY, dirty),
                (ID_SAVE, dirty),
                (ID_RESET_PAGE, page != GROUP_ABOUT),
            ] {
                let control = get_control(hwnd, id);
                if !control.is_null() {
                    EnableWindow(control, on as i32);
                }
            }
        }
    }

    /// Preset reset: loads factory defaults into the draft and refreshes
    /// the controls. Nothing reaches the runtime or disk until Apply/Save.
    /// The draft is cloned out before reloading controls: reloading writes
    /// control text, which fires EN_CHANGE, which must never reenter a
    /// live editor borrow.
    unsafe fn handle_reset(hwnd: HWND, section: Option<Section>) {
        unsafe {
            let draft = EDITOR.with(|cell| {
                cell.borrow_mut().as_mut().map(|editor| {
                    match section {
                        Some(group) => editor.reset_section(group),
                        None => editor.reset_all(),
                    }
                    editor.draft().clone()
                })
            });
            if let Some(draft) = draft {
                load_fields(hwnd, &draft);
            }
            set_text(hwnd, ID_BINDING_ERROR, "");
            set_text(hwnd, ID_GRID_ERROR, "");
            set_text(
                hwnd,
                ID_MESSAGE,
                "Defaults loaded. Apply or Save to keep them.",
            );
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => unsafe {
                let mut paint = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                0
            },
            WM_VSCROLL => unsafe {
                scroll_command(hwnd, (wparam & 0xFFFF) as i32);
                0
            },
            WM_MOVE => unsafe {
                save_placement(hwnd);
                0
            },
            WM_GETMINMAXINFO => unsafe {
                // Fixed 820x640 today; the tracked minimum honors the brief
                // for when resizing lands.
                #[allow(clippy::cast_ptr_alignment)]
                let info = &mut *(lparam as *mut MINMAXINFO);
                info.ptMinTrackSize.x = SETTINGS_MIN_WIDTH;
                info.ptMinTrackSize.y = SETTINGS_MIN_HEIGHT;
                0
            },
            WM_MOUSEWHEEL => unsafe {
                let delta = ((wparam >> 16) as u16 as i16) as i32;
                let parent = GetParent(hwnd);
                let mut info = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_POS,
                    ..Default::default()
                };
                GetScrollInfo(parent, SB_VERT, &mut info);
                scroll_to(parent, info.nPos - delta / 120 * 84);
                0
            },
            WM_COMMAND => unsafe {
                let id = (wparam & 0xFFFF) as i32;
                if id == ID_SEARCH && ((wparam >> 16) & 0xFFFF) as u32 == EN_CHANGE {
                    apply_search(hwnd, &get_text(hwnd, ID_SEARCH));
                    refresh_footer(hwnd);
                    return 0;
                }
                if (ID_PAGE_GENERAL..ID_PAGE_GENERAL + GROUP_COUNT as i32).contains(&id) {
                    set_text(hwnd, ID_SEARCH, "");
                    show_page(hwnd, (id - ID_PAGE_GENERAL) as usize);
                    refresh_footer(hwnd);
                    return 0;
                }
                // Every value change reloads the draft first, so Apply/Save
                // enable exactly while the draft differs from applied. The
                // search box is excluded: typing there edits no setting.
                let code = ((wparam >> 16) & 0xFFFF) as u32;
                if code == EN_SETFOCUS
                    && let Some(detail) = detail_for_control(id)
                {
                    set_text(hwnd, ID_MESSAGE, detail);
                }
                if code == EN_CHANGE || code == BN_CLICKED || code == CBN_SELCHANGE {
                    sync_draft(hwnd);
                }
                if (MOUSE_RECORD_BASE..MOUSE_RECORD_BASE + mouse_row_count() as i32).contains(&id) {
                    let row = id - MOUSE_RECORD_BASE;
                    RECORDING_KEY.with(|recording| recording.set(Some(MOUSE_KEY_BASE + row)));
                    SetFocus(hwnd);
                    set_text(hwnd, ID_MESSAGE, "Press a key to record. Esc cancels.");
                    return 0;
                }
                if (MOUSE_CLEAR_BASE..MOUSE_CLEAR_BASE + mouse_row_count() as i32).contains(&id) {
                    let row = id - MOUSE_CLEAR_BASE;
                    RECORDING_KEY.with(|recording| recording.set(None));
                    set_text(hwnd, MOUSE_KEY_BASE + row, "");
                    set_text(hwnd, ID_MESSAGE, "Key cleared.");
                    return 0;
                }
                match id {
                    ID_APPLY => handle_apply(hwnd, false),
                    ID_SAVE => handle_apply(hwnd, true),
                    ID_RESET_SETTINGS => handle_reset(hwnd, Some(Section::Settings)),
                    ID_RESET_APPEARANCE => handle_reset(hwnd, Some(Section::Appearance)),
                    ID_RESET_BINDINGS => handle_reset(hwnd, Some(Section::Bindings)),
                    ID_RESET_GRID => handle_reset(hwnd, Some(Section::Grid)),
                    ID_RESET_ALL => handle_reset(hwnd, None),
                    ID_COPY_DIAG => handle_diagnostics(hwnd, true),
                    ID_OPEN_LOG => handle_diagnostics(hwnd, false),
                    ID_RESET_PAGE => {
                        let page = CURRENT_PAGE.with(Cell::get);
                        let section = match page {
                            GROUP_GENERAL | GROUP_MOVEMENT => Some(Section::Settings),
                            GROUP_GRID => Some(Section::Grid),
                            GROUP_SHORTCUTS => Some(Section::Bindings),
                            GROUP_APPEARANCE => Some(Section::Appearance),
                            _ => None,
                        };
                        if let Some(section) = section {
                            handle_reset(hwnd, Some(section));
                        }
                    }
                    ID_PRACTICE_AGAIN => {
                        crate::practice_dialog::PRACTICE_OPEN_REQUEST
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                        set_text(hwnd, ID_MESSAGE, "Practice opens on the event loop.");
                    }
                    ID_CANCEL => {
                        let draft = EDITOR.with(|cell| {
                            cell.borrow_mut().as_mut().map(|editor| {
                                editor.cancel();
                                editor.draft().clone()
                            })
                        });
                        if let Some(draft) = draft {
                            load_fields(hwnd, &draft);
                        }
                        ShowWindow(hwnd, SW_HIDE);
                    }
                    _ => {}
                }
                refresh_footer(hwnd);
                0
            },
            WM_KEYDOWN => unsafe {
                if let Some(target) = RECORDING_KEY.with(Cell::get) {
                    if wparam as u32 == 0x1B {
                        RECORDING_KEY.with(|recording| recording.set(None));
                        set_text(hwnd, ID_MESSAGE, "Recording cancelled.");
                    } else if let Some(key) = crate::scancode::vk_to_logical(wparam as u32) {
                        RECORDING_KEY.with(|recording| recording.set(None));
                        set_text(hwnd, target, clickless_config::logical_key_name(key));
                        set_text(hwnd, ID_MESSAGE, "Key recorded.");
                    } else {
                        set_text(hwnd, ID_MESSAGE, "Unsupported key. Esc cancels recording.");
                    }
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            },
            WM_CLOSE => {
                // Dirty close asks; clean close hides to tray. The dialog
                // itself is intentionally thin and native-verified only.
                unsafe {
                    let dirty = EDITOR.with(|cell| {
                        cell.borrow()
                            .as_ref()
                            .is_some_and(|editor| editor.is_dirty())
                    });
                    if dirty {
                        let answer = MessageBoxW(
                            hwnd,
                            wide("Save changes?").as_ptr(),
                            wide("Clickless Settings").as_ptr(),
                            MB_YESNOCANCEL | MB_ICONQUESTION,
                        );
                        if answer == IDYES {
                            handle_apply(hwnd, true);
                            refresh_footer(hwnd);
                            ShowWindow(hwnd, SW_HIDE);
                        } else if answer == IDNO {
                            let draft = EDITOR.with(|cell| {
                                cell.borrow_mut().as_mut().map(|editor| {
                                    editor.cancel();
                                    editor.draft().clone()
                                })
                            });
                            if let Some(draft) = draft {
                                load_fields(hwnd, &draft);
                            }
                            refresh_footer(hwnd);
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    } else {
                        ShowWindow(hwnd, SW_HIDE);
                    }
                }
                0
            }
            WM_DESTROY => 0,
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    /// One settings window per process. Reopening brings the same window back.
    pub struct SettingsWindow {
        hwnd: HWND,
    }

    impl SettingsWindow {
        pub fn new() -> Result<Self, String> {
            Self::with_on_apply(Box::new(|_| Ok(())))
        }

        /// Creates the editor window. `on_apply` receives the validated
        /// config after a successful Apply; the runtime push happens on the
        /// event-loop thread inside this callback.
        pub fn with_on_apply(on_apply: ApplyCallback) -> Result<Self, String> {
            register_class()?;
            let class_name = wide(CLASS_NAME);
            let title = wide("Clickless Settings");
            // Restored position when valid, centered 820x640 first open.
            let (screen_w, screen_h) =
                unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
            let at = load_placement(screen_w, screen_h);
            // The 820x640 design size is 96-dpi; the frame scales with the
            // system DPI so a 150% session gets a proportionally wider window.
            let dpi = unsafe { GetDpiForSystem() } as i32;
            let width = SETTINGS_WIDTH * dpi / 96;
            let height = SETTINGS_HEIGHT * dpi / 96;
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VSCROLL | WS_CLIPCHILDREN,
                    at.x,
                    at.y,
                    width,
                    height,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                return Err("CreateWindowExW failed for the settings window".to_string());
            }
            unsafe {
                // Dark title bar. Best effort: ignored on builds without the
                // attribute; the window still works, just not dark chrome.
                let dark: i32 = 1;
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    &dark as *const i32 as *const core::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );
            }
            unsafe {
                // Seed the editor from the last applied config. The CLI sets
                // this before opening the window; defaults are the fallback.
                let initial = SETTINGS_SEED
                    .with(|cell| cell.borrow_mut().take())
                    .unwrap_or_default();
                MOUSE_ROW_COUNT.with(|count| {
                    count.set(
                        initial
                            .mouse_bindings
                            .len()
                            .max(DEFAULT_MOUSE_BINDING_SLOTS),
                    )
                });
                let (_viewport, content, viewport_height, content_width) =
                    create_scroll_surfaces(hwnd);
                // Global chrome on the top-level window: never tagged into
                // the content registry, never filtered, never moved.
                CURRENT_GROUP.with(|group| group.set(NO_GROUP));
                add_row(hwnd, ID_SEARCH, "Search", 8);
                let mut side_y = TOP_STRIP_HEIGHT + 4;
                for (index, name) in PAGE_NAMES.iter().enumerate() {
                    create_child(
                        hwnd,
                        "BUTTON",
                        name,
                        WS_TABSTOP,
                        PAGE_IDS[index],
                        12,
                        side_y,
                        130,
                        26,
                    );
                    side_y += 32;
                }
                let footer_y = TOP_STRIP_HEIGHT + viewport_height;
                let content_height = build_controls(content, hwnd, footer_y);
                #[cfg(test)]
                ensure_test_help_controls(content);
                SetWindowPos(
                    content,
                    null_mut(),
                    0,
                    0,
                    content_width,
                    content_height,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS,
                );
                set_scroll_content(hwnd, content_height, viewport_height);
                EDITOR.with(|cell| *cell.borrow_mut() = Some(SettingsEditor::new(initial.clone())));
                load_fields(hwnd, &initial);
                show_page(hwnd, GROUP_GENERAL);
                refresh_footer(hwnd);
                // One UI font for the whole tree, process lifetime.
                let dpi = GetDpiForWindow(hwnd);
                let (font, _) = create_ui_font(dpi);
                if !font.is_null() {
                    EnumChildWindows(hwnd, Some(apply_font_child), font as LPARAM);
                }
                if let Some(notice) = NOTICE_SEED.with(|cell| cell.borrow_mut().take()) {
                    set_text(hwnd, ID_MESSAGE, &notice);
                }
                ON_APPLY.with(|cell| *cell.borrow_mut() = Some(on_apply));
            }
            Ok(Self { hwnd })
        }

        pub fn show(&self) {
            unsafe {
                // Every tray/second-launch open starts at the top and raises
                // the existing window instead of leaving it behind the app
                // the user was working in.
                scroll_to(self.hwnd, 0);
                ShowWindow(self.hwnd, SW_RESTORE);
                BringWindowToTop(self.hwnd);
                SetForegroundWindow(self.hwnd);
                // Reopen lands keyboard focus on the first useful control.
                SetFocus(get_control(self.hwnd, ID_ENABLED));
            }
        }

        pub fn has_focus(&self) -> bool {
            unsafe { GetForegroundWindow() == self.hwnd }
        }

        pub fn is_visible(&self) -> bool {
            unsafe { IsWindowVisible(self.hwnd) != 0 }
        }

        pub fn translate_message(&self, msg: &MSG) -> bool {
            if RECORDING_KEY.with(Cell::get).is_some() {
                return false;
            }
            unsafe {
                IsDialogMessageW(CONTENT_WINDOW.with(Cell::get), msg) != 0
                    || IsDialogMessageW(VIEWPORT_WINDOW.with(Cell::get), msg) != 0
                    || IsDialogMessageW(self.hwnd, msg) != 0
            }
        }

        pub fn hide(&self) {
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }

        pub fn is_created(&self) -> bool {
            !self.hwnd.is_null()
        }
    }

    #[cfg(test)]
    unsafe fn ensure_test_help_controls(parent: HWND) {
        unsafe {
            for entry in crate::settings_help::ALL {
                let Some(id) = help_control_id(entry.id) else {
                    continue;
                };
                if GetDlgItem(parent, id).is_null() {
                    let control = create_child(parent, "STATIC", entry.short, 0, id, 0, 0, 1, 1);
                    SetWindowLongPtrW(control, GWLP_USERDATA, 1);
                }
                let input = get_control(parent, input_control_id(entry.id).unwrap_or_default());
                if !input.is_null() {
                    SetWindowLongPtrW(input, GWLP_USERDATA, 1);
                }
            }
        }
    }

    thread_local! {
        /// Set by the CLI before the window is created so the editor starts
        /// from the running config, not defaults.
        static SETTINGS_SEED: RefCell<Option<clickless_config::Config>> = const { RefCell::new(None) };
        /// Malformed-startup explanation, shown once in the message line.
        static NOTICE_SEED: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    /// Supplies the running config so the editor opens on real values.
    pub fn seed_settings(config: clickless_config::Config) {
        SETTINGS_SEED.with(|cell| *cell.borrow_mut() = Some(config));
    }

    /// Supplies a startup notice (malformed config) shown in the window.
    pub fn seed_notice(notice: String) {
        NOTICE_SEED.with(|cell| *cell.borrow_mut() = Some(notice));
    }

    /// Takes the pending startup notice, if any. A host that draws its own
    /// status line calls this instead of leaving the notice for another window.
    pub fn take_notice() -> Option<String> {
        NOTICE_SEED.with(|cell| cell.borrow_mut().take())
    }

    impl Drop for SettingsWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.hwnd) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::Graphics::Gdi::{GetUpdateRect, InvalidateRect, UpdateWindow};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetFocus, IsWindowEnabled, SetFocus,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            BM_CLICK, BS_DEFPUSHBUTTON, GWL_STYLE, GetWindowLongPtrW, GetWindowRect,
            IsWindowVisible, WM_KEYDOWN,
        };

        #[test]
        fn settings_controls_toggle_accept_edits_and_handle_tab() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                for id in [
                    ID_ENABLED,
                    ID_SEARCH,
                    ID_PAGE_GENERAL,
                    ID_PAGE_MOVEMENT,
                    ID_PAGE_GRID,
                    ID_PAGE_SHORTCUTS,
                    ID_PAGE_APPEARANCE,
                    ID_PAGE_ABOUT,
                    ID_RESET_PAGE,
                    ID_MOTION_LABEL,
                    ID_APPEAR_LABEL,
                    ID_LEADER,
                    ID_START_SPEED,
                    ID_MAX_SPEED,
                    ID_RAMP,
                    ID_HOLD_MS,
                    ID_SCROLL_STEP,
                    ID_RESET_SETTINGS,
                    ID_RESET_APPEARANCE,
                    ID_RESET_BINDINGS,
                    ID_RESET_GRID,
                    ID_RESET_ALL,
                    ID_COPY_DIAG,
                    ID_OPEN_LOG,
                    ID_PRACTICE_AGAIN,
                    ID_LAYOUT,
                    ID_GRID_ROWS,
                    ID_GRID_COLS,
                    ID_GRID_KEYS,
                    ID_COLUMN_KEYS,
                    ID_ROW_KEYS,
                    ID_APPLY,
                    ID_SAVE,
                    ID_CANCEL,
                ] {
                    assert!(!get_control(window.hwnd, id).is_null(), "control {id}");
                }
                assert_eq!(
                    IsWindowEnabled(get_control(window.hwnd, ID_APPLY)),
                    0,
                    "apply disabled when clean"
                );
                assert_eq!(
                    GetWindowLongPtrW(get_control(window.hwnd, ID_SAVE), GWL_STYLE) as u32 & 0xF,
                    BS_DEFPUSHBUTTON as u32,
                    "save is the primary button"
                );
                for id in [ID_ENABLED, ID_NUDGE_ENABLE, ID_DRAG, ID_AUTO_FREE] {
                    let control = get_control(window.hwnd, id);
                    assert_eq!(
                        GetWindowLongPtrW(control, GWL_STYLE) as u32 & 0xF,
                        BS_AUTOCHECKBOX as u32,
                        "checkbox style for {id}"
                    );
                    let was_checked = get_check(window.hwnd, id);
                    SendMessageW(control, BM_CLICK, 0, 0);
                    assert_eq!(get_check(window.hwnd, id), !was_checked, "toggle {id}");
                }
                assert_eq!(get_text(window.hwnd, ID_MOTION_LABEL), "General");
                assert_eq!(get_text(window.hwnd, ID_APPEAR_LABEL), "Appearance");
                let leader = get_control(window.hwnd, ID_LEADER);
                let hold = get_control(window.hwnd, ID_HOLD_MS);
                SetFocus(leader);
                assert_eq!(GetFocus(), leader);
                let mut tab: MSG = std::mem::zeroed();
                tab.hwnd = leader;
                tab.message = WM_KEYDOWN;
                tab.wParam = 0x09;
                assert!(window.translate_message(&tab));
                assert_eq!(GetFocus(), hold);
                SetWindowTextW(leader, wide("F8").as_ptr());
                assert_eq!(read_fields(window.hwnd).leader, "F8");
                SetWindowTextW(get_control(window.hwnd, ID_HOLD_MS), wide("350").as_ptr());
                SetWindowTextW(get_control(window.hwnd, ID_SCROLL_STEP), wide("3").as_ptr());
                let reread = read_fields(window.hwnd);
                assert_eq!(reread.hold_ms, "350");
                assert_eq!(reread.scroll_step, "3");
                SendMessageW(get_control(window.hwnd, ID_RESET_ALL), BM_CLICK, 0, 0);
                let reset = read_fields(window.hwnd);
                assert_eq!(reset.hold_ms, "200");
                assert_eq!(reset.scroll_step, "1");
                assert_eq!(
                    reset.grid_keys.len(),
                    9,
                    "simple grid keys survive the list round-trip"
                );
                let binding_key = get_control(window.hwnd, MOUSE_KEY_BASE);
                SendMessageW(get_control(window.hwnd, MOUSE_RECORD_BASE), BM_CLICK, 0, 0);
                assert_eq!(RECORDING_KEY.with(Cell::get), Some(MOUSE_KEY_BASE));
                let esc = MSG {
                    hwnd: window.hwnd,
                    message: WM_KEYDOWN,
                    wParam: 0x1B,
                    ..std::mem::zeroed()
                };
                assert!(!window.translate_message(&esc));
                wnd_proc(window.hwnd, WM_KEYDOWN, 0x1B, 0);
                assert_eq!(RECORDING_KEY.with(Cell::get), None);
                SendMessageW(get_control(window.hwnd, MOUSE_RECORD_BASE), BM_CLICK, 0, 0);
                wnd_proc(window.hwnd, WM_KEYDOWN, b'G' as usize, 0);
                assert_eq!(get_text(window.hwnd, MOUSE_KEY_BASE), "g");
                SendMessageW(get_control(window.hwnd, MOUSE_CLEAR_BASE), BM_CLICK, 0, 0);
                assert_eq!(get_text(window.hwnd, MOUSE_KEY_BASE), "");
                // Pages fit the viewport, so the scrollbar has no range: page
                // switches reset scroll position instead of moving content.
                SendMessageW(get_control(window.hwnd, ID_PAGE_GRID), BM_CLICK, 0, 0);
                let content = GetDlgItem(VIEWPORT_WINDOW.with(Cell::get), 702);
                let mut content_before = RECT::default();
                let mut content_after = RECT::default();
                let apply_button = get_control(window.hwnd, ID_APPLY);
                let viewport = VIEWPORT_WINDOW.with(Cell::get);
                let mut footer_before = RECT::default();
                let mut footer_after = RECT::default();
                GetWindowRect(content, &mut content_before);
                GetWindowRect(apply_button, &mut footer_before);
                scroll_to(window.hwnd, i32::MAX);
                scroll_to(window.hwnd, 0);
                SendMessageW(get_control(window.hwnd, ID_PAGE_GENERAL), BM_CLICK, 0, 0);
                SendMessageW(get_control(window.hwnd, ID_PAGE_GRID), BM_CLICK, 0, 0);
                GetWindowRect(content, &mut content_after);
                GetWindowRect(apply_button, &mut footer_after);
                assert_eq!(
                    (content_after.left, content_after.top),
                    (content_before.left, content_before.top),
                    "scroll plus page switches never drift content"
                );
                assert_eq!(
                    footer_after.top, footer_before.top,
                    "footer stays fixed across pages"
                );
                assert_eq!(
                    GetUpdateRect(viewport, null_mut(), 0),
                    0,
                    "page switch leaves no pending paint"
                );
                SendMessageW(get_control(window.hwnd, ID_PAGE_GENERAL), BM_CLICK, 0, 0);
                assert_eq!(
                    IsWindowVisible(get_control(window.hwnd, ID_GRID_ROWS)),
                    0,
                    "leaving Grid hides its rows"
                );
                assert!(!binding_key.is_null());
                assert_eq!(read_fields(window.hwnd).leader, "capslock");
                apply_search(window.hwnd, "scroll");
                assert_ne!(
                    IsWindowVisible(get_control(window.hwnd, ID_SCROLL_STEP)),
                    0,
                    "matching group stays visible"
                );
                assert_eq!(
                    IsWindowVisible(get_control(window.hwnd, ID_PANEL)),
                    0,
                    "other groups hide"
                );
                apply_search(window.hwnd, "");
                assert_ne!(
                    IsWindowVisible(get_control(window.hwnd, ID_LEADER)),
                    0,
                    "clearing search restores the General page"
                );
                assert_eq!(
                    IsWindowVisible(get_control(window.hwnd, ID_PANEL)),
                    0,
                    "other pages stay hidden"
                );
                SendMessageW(get_control(window.hwnd, ID_PAGE_GRID), BM_CLICK, 0, 0);
                assert_ne!(
                    IsWindowVisible(get_control(window.hwnd, ID_GRID_ROWS)),
                    0,
                    "sidebar opens the Grid page"
                );
                assert_eq!(
                    IsWindowVisible(get_control(window.hwnd, ID_LEADER)),
                    0,
                    "General hides behind Grid"
                );
                assert_eq!(binding_key, get_control(window.hwnd, MOUSE_KEY_BASE));
                InvalidateRect(window.hwnd, null(), 1);
                assert_ne!(GetUpdateRect(window.hwnd, null_mut(), 0), 0);
                UpdateWindow(window.hwnd);
                assert_eq!(GetUpdateRect(window.hwnd, null_mut(), 0), 0);
                // Live sync dirties the draft on edit; Cancel cleans again.
                SetWindowTextW(get_control(window.hwnd, ID_HOLD_MS), wide("350").as_ptr());
                assert_ne!(
                    IsWindowEnabled(get_control(window.hwnd, ID_APPLY)),
                    0,
                    "dirty draft enables apply"
                );
                SendMessageW(get_control(window.hwnd, ID_CANCEL), BM_CLICK, 0, 0);
                assert_eq!(
                    IsWindowEnabled(get_control(window.hwnd, ID_APPLY)),
                    0,
                    "cancel restores clean disabled"
                );
            }
        }

        #[test]
        fn placement_parses_centers_and_clamps() {
            assert_eq!(
                parse_placement("100 200"),
                Some(Placement { x: 100, y: 200 })
            );
            assert_eq!(parse_placement("nope"), None);
            assert_eq!(parse_placement("1 2 3"), None);
            let rendered = render_placement(Placement { x: 5, y: 6 });
            assert_eq!(parse_placement(&rendered), Some(Placement { x: 5, y: 6 }));
            assert_eq!(default_placement(1920, 1080), Placement { x: 550, y: 220 });
            assert_eq!(
                clamp_placement(Placement { x: -5000, y: -99 }, 1920, 1080),
                Placement { x: 100 - 820, y: 0 }
            );
            assert_eq!(
                clamp_placement(Placement { x: 5000, y: 5000 }, 1920, 1080),
                Placement { x: 1820, y: 980 }
            );
        }

        #[test]
        fn ui_font_resolves_to_a_supported_face() {
            use windows_sys::Win32::Graphics::Gdi::{DeleteObject, GetDC, ReleaseDC};
            let (_font, face) = unsafe { create_ui_font(96) };
            eprintln!("probe face={face:?}");
            assert!(
                ["Segoe UI Variable Text", "Segoe UI"].contains(&face.as_str()),
                "unexpected face {face:?}"
            );
            unsafe {
                let dc = GetDC(null_mut());
                ReleaseDC(null_mut(), dc);
                DeleteObject(_font);
            }
        }

        #[test]
        fn settings_notice_seed_shows_startup_error() {
            seed_settings(clickless_config::Config::default());
            seed_notice("Config ignored: hold_ms must be above 0.".to_string());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                assert_eq!(
                    get_text(window.hwnd, ID_MESSAGE),
                    "Config ignored: hold_ms must be above 0."
                );
            }
        }

        #[test]
        fn settings_window_is_modern_and_dark() {
            use windows_sys::Win32::Graphics::Dwm::{
                DWMWA_USE_IMMERSIVE_DARK_MODE, DwmGetWindowAttribute,
            };
            use windows_sys::Win32::UI::Controls::{CloseThemeData, OpenThemeData};
            use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
            use windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics;

            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                // Visual styles: comctl32 v6 via an activation context, or a
                // manifest. OpenThemeData serves themed handles only when
                // the v6 context is active; the legacy context returns null
                // and every control renders 1995-gray.
                let theme = OpenThemeData(window.hwnd, wide("BUTTON").as_ptr());
                assert_ne!(theme, 0, "comctl32 v6 visual styles must be active");
                CloseThemeData(theme);
                // Dark title bar attribute must be set on the window.
                let mut data: i32 = 0;
                let size = std::mem::size_of::<i32>() as u32;
                let hr = DwmGetWindowAttribute(
                    window.hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    &mut data as *mut i32 as *mut core::ffi::c_void,
                    size,
                );
                assert_eq!(hr, 0, "DWM dark-mode attribute readable");
                assert_eq!(data, 1, "dark title bar requested");
                // Layout must scale with DPI: a 150% (144 dpi) session needs
                // a window wider than the 96-dpi 820 design width.
                let dpi = GetDpiForWindow(window.hwnd);
                let mut rect = RECT::default();
                GetWindowRect(window.hwnd, &mut rect);
                let expected = (820 * dpi as i32) / 96;
                let screen = GetSystemMetrics(SM_CXSCREEN);
                assert_eq!(
                    rect.right - rect.left,
                    expected.min(screen),
                    "window width must track DPI"
                );
            }
        }

        #[test]
        fn settings_rows_carry_persistent_help_and_focus_detail() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                // Every shipped setting row shows a persistent short sentence.
                for entry in crate::settings_help::ALL {
                    let id = help_control_id(entry.id).expect("help entry maps to a control");
                    let short = get_control(window.hwnd, id);
                    assert!(!short.is_null(), "missing help line for {}", entry.id);
                    assert_eq!(get_text(window.hwnd, id), entry.short);
                    let control = match input_control_id(entry.id) {
                        Some(input_id) => get_control(window.hwnd, input_id),
                        None => short,
                    };
                    assert_ne!(
                        GetWindowLongPtrW(control, GWLP_USERDATA) as usize,
                        0,
                        "detail hover/focus target missing for {}",
                        entry.id
                    );
                }
                // Keyboard focus on any input shows the same detail.
                SetFocus(get_control(window.hwnd, ID_LEADER));
                assert_eq!(
                    get_text(window.hwnd, ID_MESSAGE),
                    "Hold CapsLock for the configured delay, then choose labels while still holding. A Left Shift tap opens the same grid without holding it. Rebinding this key steals it while held.",
                    "focus must surface the detail sentence"
                );
            }
        }

        #[test]
        fn settings_validation_focuses_first_invalid_field() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                SetFocus(get_control(window.hwnd, ID_ENABLED));
                SetWindowTextW(get_control(window.hwnd, ID_MAX_SPEED), wide("10").as_ptr());
                sync_draft(window.hwnd);
                assert_eq!(
                    get_text(window.hwnd, ID_MESSAGE),
                    "max speed 10 must be at least the start speed 300",
                    "error must name the field and the problem"
                );
                SendMessageW(get_control(window.hwnd, ID_APPLY), BM_CLICK, 0, 0);
                assert_eq!(
                    GetFocus(),
                    get_control(window.hwnd, ID_MAX_SPEED),
                    "Apply must focus the first invalid field"
                );
            }
        }

        #[test]
        fn settings_never_shows_internal_tokens_or_dead_note() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                for row in 0..mouse_row_count() {
                    set_combo(window.hwnd, MOUSE_ACTION_BASE + row as i32, "click_left");
                    // The combo itself never displays the token: the visible
                    // selection text is the human phrase.
                    let combo = get_control(window.hwnd, MOUSE_ACTION_BASE + row as i32);
                    assert_eq!(
                        get_combo(window.hwnd, MOUSE_ACTION_BASE + row as i32),
                        "click_left",
                        "value stays the canonical token"
                    );
                    let mut text = [0u16; 64];
                    GetWindowTextW(combo, text.as_mut_ptr(), 64);
                    let shown = String::from_utf16_lossy(&text);
                    let shown = shown.split('\0').next().unwrap_or("");
                    assert!(
                        !shown.contains("click_left"),
                        "combo displays token {shown:?} to the user"
                    );
                    assert_eq!(shown, "Left click");
                }
                // Dead note never renders.
                assert_eq!(
                    get_text(window.hwnd, ID_INITIAL_BINDINGS_NOTE),
                    "",
                    "initial-layer note must stay empty"
                );
                // Layout choice is a combo, not a free-text edit.
                let style =
                    GetWindowLongPtrW(get_control(window.hwnd, ID_LAYOUT), GWL_STYLE) as u32;
                assert_eq!(
                    style & 0xF,
                    CBS_DROPDOWNLIST as u32 & 0xF,
                    "layout is a dropdown"
                );
            }
        }

        #[test]
        fn reopening_settings_focuses_first_useful_control() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                window.show();
                assert_eq!(
                    GetFocus(),
                    get_control(window.hwnd, ID_ENABLED),
                    "reopen must focus the first useful control"
                );
            }
        }

        #[test]
        fn reopening_settings_resets_scroll_and_shows_the_window() {
            use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                // Force a real scroll range independent of which page is active.
                set_scroll_content(window.hwnd, 2_000, 200);
                scroll_to(window.hwnd, i32::MAX);
                let mut before = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_POS,
                    ..Default::default()
                };
                GetScrollInfo(window.hwnd, SB_VERT, &mut before);
                assert!(before.nPos > 0, "test setup must scroll away from top");

                window.show();

                let mut after = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_POS,
                    ..Default::default()
                };
                GetScrollInfo(window.hwnd, SB_VERT, &mut after);
                assert_eq!(after.nPos, 0, "reopen starts at the top");
                assert_ne!(IsWindowVisible(window.hwnd), 0);
            }
        }

        #[test]
        fn settings_hint_text_stays_above_next_row() {
            seed_settings(clickless_config::Config::default());
            let window = SettingsWindow::new().expect("create Settings window");
            unsafe {
                window.show();
                let hint = get_control(window.hwnd, ID_LEADER + 2500);
                let next_edit = get_control(window.hwnd, ID_HOLD_MS);
                assert!(!hint.is_null(), "missing Leader key hint");
                assert!(!next_edit.is_null(), "missing Hold (ms) edit");
                let mut hint_rect = RECT::default();
                let mut next_rect = RECT::default();
                GetWindowRect(hint, &mut hint_rect);
                GetWindowRect(next_edit, &mut next_rect);
                assert!(
                    hint_rect.bottom <= next_rect.top,
                    "hint text must not overlap the next row"
                );
            }
        }
    }
}

#[cfg(windows)]
pub use win::{SettingsWindow, seed_notice, seed_settings};
