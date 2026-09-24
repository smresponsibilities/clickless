//! Setting help content (app-rebuild 17): plain labels, one persistent
//! short sentence, hover/focus detail, and units or examples per setting.
//! Pure data, no OS calls: any shell (Win32, Slint) renders these strings.
//! Gap rows from docs/config-coverage.md deliberately have no entry here,
//! so no shell can render a control with no runtime effect.

/// One setting's user-facing help. Short shows always; detail on hover or
/// keyboard focus; example carries units, bounds or an example value.
pub struct SettingHelp {
    pub id: &'static str,
    pub label: &'static str,
    pub short: &'static str,
    pub detail: &'static str,
    pub example: &'static str,
}

pub const ALL: &[SettingHelp] = &[
    SettingHelp {
        id: "enabled",
        label: "Enabled",
        short: "Master switch for pointer control.",
        detail: "When off, Clickless ignores all keys and hides the grid. The tray icon stays so you can turn it back on.",
        example: "On or off.",
    },
    SettingHelp {
        id: "leader",
        label: "Activation key",
        short: "Hold CapsLock to keep the grid open; tap Left Shift to open it once.",
        detail: "Hold CapsLock for the configured delay, then choose labels while still holding. A Left Shift tap opens the same grid without holding it. Rebinding this key steals it while held.",
        example: "Example: CapsLock hold; Left Shift tap.",
    },
    SettingHelp {
        id: "layout",
        label: "Grid style",
        short: "Dense grid: type 2 letters to choose an area, then 1 letter for the target.",
        detail: "Dense suits large monitors with 300 labeled areas. Simple keeps the older 3 by 3 workflow.",
        example: "Dense or Simple.",
    },
    SettingHelp {
        id: "start_speed",
        label: "Start speed",
        short: "Pointer speed when movement begins.",
        detail: "Pixels per second before acceleration. Lower values improve small adjustments.",
        example: "Pixels per second, at least 1 and at most the maximum speed. Default 300.",
    },
    SettingHelp {
        id: "max_speed",
        label: "Maximum speed",
        short: "Fastest pointer speed after ramp-up.",
        detail: "Pixels per second once acceleration finishes. Must stay at or above the start speed.",
        example: "Pixels per second. Default 3000.",
    },
    SettingHelp {
        id: "ramp_ms",
        label: "Ramp duration",
        short: "Time taken to accelerate from start to maximum speed.",
        detail: "Shorter ramps feel snappier but overshoot small targets; longer ramps land precisely.",
        example: "Milliseconds, at least 1. Default 500.",
    },
    SettingHelp {
        id: "hold_ms",
        label: "Hold delay",
        short: "How long to hold the activation key before pointer mode starts.",
        detail: "Shorter delays respond faster but mistake taps for holds; longer delays keep quick taps safe.",
        example: "Milliseconds, at least 1. Default 200.",
    },
    SettingHelp {
        id: "scroll_step",
        label: "Scroll step",
        short: "Scroll notches per scroll action.",
        detail: "Each scroll keypress moves this many notches. Higher values scroll long pages faster.",
        example: "Notches, at least 1. Default 1.",
    },
    SettingHelp {
        id: "mouse_bindings",
        label: "Pointer keys",
        short: "Keys that move, click and scroll while the activation key is held.",
        detail: "Each row pairs one key with one pointer action. Keys must be unique; Esc and Backspace are reserved and rejected.",
        example: "Example: J moves right, F left-clicks.",
    },
    SettingHelp {
        id: "column_keys",
        label: "Outer columns",
        short: "First letter of a two-letter grid choice.",
        detail: "One key per grid column. Keys must be unique and printable.",
        example: "Dense default: A S D F G H J K L semicolon.",
    },
    SettingHelp {
        id: "row_keys",
        label: "Outer rows",
        short: "Second letter of a two-letter grid choice.",
        detail: "One key per grid row. The count must match the grid rows.",
        example: "Dense default: Q W E R T Y U I O P and more.",
    },
    SettingHelp {
        id: "nested_size",
        label: "Subgrid size",
        short: "Rows and columns of the fine-selection grid.",
        detail: "The subgrid opens inside the chosen outer cell. The nested key count must equal rows times columns.",
        example: "Dense default: 3 rows by 10 columns.",
    },
    SettingHelp {
        id: "nested_keys",
        label: "Subgrid keys",
        short: "Keys that pick the final target inside a cell.",
        detail: "One unique key per subgrid cell, laid out in reading order.",
        example: "Dense default: QWERTY bank of 30 keys.",
    },
    SettingHelp {
        id: "auto_free",
        label: "Return to pointer mode",
        short: "Move without clicking, then continue keyboard pointer control.",
        detail: "After the final grid key the pointer moves but no click happens, and control returns to pointer mode.",
        example: "On or off. Default off.",
    },
    SettingHelp {
        id: "nudge_enabled",
        label: "Nudge after selection",
        short: "Keep the target open so movement keys can make pixel adjustments.",
        detail: "With nudge on, the final key moves the pointer and releasing it clicks. With nudge off, selection clicks immediately or returns to pointer mode.",
        example: "On or off. Default on.",
    },
    SettingHelp {
        id: "nudge_step",
        label: "Nudge step",
        short: "Pixels moved per nudge keypress.",
        detail: "Smaller steps land precisely on tiny targets; larger steps cross the screen faster.",
        example: "Pixels, at least 1. Dense default 1, simple default 5.",
    },
    SettingHelp {
        id: "drag_after_select",
        label: "Drag after selection",
        short: "Hold the mouse button after the final grid key. Release capture to end the drag.",
        detail: "The left button stays down at the selected target so pointer keys drag windows or select text. Esc or activation-key release ends the drag.",
        example: "On or off. Default off.",
    },
    SettingHelp {
        id: "color_panel",
        label: "Panel color",
        short: "Background of grid labels.",
        detail: "Opaque badges keep labels readable over any window behind the overlay.",
        example: "Hex RRGGBB. Default 181C26.",
    },
    SettingHelp {
        id: "color_border",
        label: "Border color",
        short: "Grid line color.",
        detail: "Single-pixel lines separating grid cells.",
        example: "Hex RRGGBB. Default 647A96.",
    },
    SettingHelp {
        id: "color_label",
        label: "Label color",
        short: "Grid letter color.",
        detail: "Light letters on the dark panel keep contrast high at small sizes.",
        example: "Hex RRGGBB. Default F0F6FF.",
    },
    SettingHelp {
        id: "color_highlight",
        label: "Highlight color",
        short: "Selected-cell highlight color.",
        detail: "Marks the cell being narrowed or the active subgrid target.",
        example: "Hex RRGGBB. Default FFC440.",
    },
    SettingHelp {
        id: "color_pointer",
        label: "Pointer color",
        short: "Target marker color.",
        detail: "Small marker drawn where the pointer will land.",
        example: "Hex RRGGBB. Default FF6060.",
    },
    SettingHelp {
        id: "opacity",
        label: "Panel and highlight opacity",
        short: "How solid the label badges and highlight are.",
        detail: "Higher values stay readable over busy windows; lower values show more of the app beneath. Sizes: label size in points, border width in pixels.",
        example: "0 to 255. Panel default 70, highlight 110, label size 3, border 1.",
    },
];

/// Help for a shippable setting id, or None for gap rows and unknowns.
pub fn for_id(id: &str) -> Option<&'static SettingHelp> {
    ALL.iter().find(|entry| entry.id == id)
}
