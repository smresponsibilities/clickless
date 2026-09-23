# Settings modernization research

Date: 2026-09-21

## Reference pattern

PowerToys Settings is a Windows App SDK / WinUI 3 application. Its shell uses a NavigationView-style rail, a dashboard with grouped modules, SettingsCard-like rows, switches, shortcut chips, descriptions, and responsive content. Microsoft recommends a navigation pane, grouped settings, constrained readable content width, descriptive labels, and a description/action pairing for each setting.

Sources:

- Microsoft, [Guidelines for app settings](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings)
- Microsoft, [NavigationView](https://learn.microsoft.com/en-us/windows/apps/design/controls/navigationview)
- Microsoft PowerToys, [settings implementation](https://github.com/microsoft/PowerToys/blob/main/doc/devdocs/core/settings/settings-implementation.md)
- Microsoft PowerToys, [new PowerToy settings integration](https://github.com/microsoft/PowerToys/blob/main/doc/devdocs/development/new-powertoy.md)

## Gap in Clickless

The current Settings window is a hand-built Win32 dialog. It has a sidebar and pages, but its controls are flat native EDIT/BUTTON/STATIC children. That explains the screenshot problems:

- no dashboard cards;
- no switches or shortcut chips;
- no consistent description/action alignment;
- no modern dark/light theme system;
- no responsive two-column layout;
- no reliable hover/tooltip model;
- input recording competes with normal dialog focus;
- content height and descriptions collide at narrow widths.

Adding more STATIC labels cannot solve those structural gaps.

## Correct implementation decision

Keep the Rust core, configuration, hook, tray, and output crates unchanged. Replace only the Settings presentation layer with a Windows-only WinUI 3 host.

Required boundary:

```text
clickless-core/config  ->  settings view model  ->  WinUI 3 Settings host
                                      |
                                      +-> Apply/Save callback on event-loop thread
```

The existing `SettingsEditor` remains the validation and transactional-save authority. WinUI owns navigation, cards, toggles, shortcut capture, descriptions, theme, DPI, and accessibility.

## Target shell

- Left NavigationView rail: Dashboard, General, Movement, Grid, Shortcuts, Appearance, About.
- Dashboard: “Clickless” title, enabled status card, activation-key card, grid workflow card, and diagnostics card.
- Pages: maximum readable width around 1000–1100 px; each setting is a card with title, one-sentence description, example/help, and right-aligned control.
- Boolean values: ToggleSwitch.
- Small exclusive choices: RadioButtons or ComboBox.
- Keyboard bindings: shortcut chip/button that enters capture mode and clearly shows “Press a key”.
- Colors: color swatch plus editable hex value.
- Save model: immediate Apply for runtime, explicit Save for disk, visible unsaved banner, and error text beside the failing card.
- Theme: follow Windows by default, with light/dark/high-contrast-safe resources.

## Explicit non-goals

- Do not add a webview.
- Do not move core behavior into UI code.
- Do not duplicate validation or config serialization.
- Do not continue expanding the Win32 dialog toward a fake Fluent clone.

## Acceptance bar

The work is not “modern” until a fresh build visibly has:

1. PowerToys-like rail and page hierarchy.
2. Cards with descriptions and examples visible without hovering.
3. Real toggles and shortcut capture controls.
4. No clipped help at 100%, 150%, or 200% DPI.
5. Keyboard-only navigation and Narrator names.
6. Apply/Save failures shown on the affected card with the old config preserved.
7. Practice launched from the dashboard with a clear three-step card.

## Recommendation

Do not spend more time tuning coordinates in `settings.rs`. First create the WinUI 3 host and move one page, General, end to end. Once General meets the acceptance bar, migrate Movement, Grid, Shortcuts, Appearance, and About one page at a time.
