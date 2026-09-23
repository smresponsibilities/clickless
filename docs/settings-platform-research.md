# Settings platform research

Date: 2026-09-21

## Windows

Use Windows App SDK / WinUI 3 for the Settings host. PowerToys uses this model. The shell should use NavigationView, a dashboard, SettingsCard-like rows, ToggleSwitch controls, shortcut capture chips, responsive content, and resource-based light/dark/high-contrast styling.

Source: [Microsoft Settings guidelines](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings), [PowerToys settings implementation](https://github.com/microsoft/PowerToys/blob/main/doc/devdocs/core/settings/settings-implementation.md).

## macOS

Use a native SwiftUI `Settings` scene when a macOS app target exists. Apple manages the Settings menu item, window presentation, and platform conventions. Use `Form`, `Section`, `Toggle`, `Picker`, `Stepper`, and a dedicated shortcut recorder. AppKit is the fallback for a non-SwiftUI host.

Source: [Apple Settings scene](https://developer.apple.com/documentation/swiftui/settings), [Adding a settings interface](https://developer.apple.com/documentation/foundation/adding-a-settings-interface).

Do not force the Windows rail onto macOS. macOS settings normally use a separate preferences window with tabs or a sidebar appropriate to the number of groups.

## Linux

Use GTK 4 with Libadwaita on GNOME desktops. `AdwPreferencesWindow` and preference groups provide the native pattern, automatic light/dark/high-contrast support, accessible controls, and adaptive layouts. On KDE, Kirigami is the native alternative when a Qt/KDE dependency is acceptable.

Sources: [GNOME HIG](https://developer.gnome.org/hig/), [Adw styles and appearance](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html), [GNOME secondary windows](https://developer.gnome.org/hig/patterns/containers/windows.html), [KDE Kirigami](https://develop.kde.org/docs/getting-started/kirigami/).

Do not assume Libadwaita exists on every Linux installation. Keep the settings model and validation in Rust, and make GTK/Libadwaita an optional Linux UI feature.

## Shared boundary

```text
clickless-core + clickless-config
          |
          +-- SettingsEditor / validation / transactional save
          |
          +-- platform view model and event-loop callback
                 |-- Windows: WinUI 3 host
                 |-- macOS: SwiftUI Settings scene or AppKit preferences
                 `-- Linux: GTK4/Libadwaita PreferencesWindow (Kirigami optional)
```

The platform shells must not duplicate parsing, validation, defaults, or save semantics. Each shell owns only presentation, input capture, accessibility, theme resources, and the native settings-window lifecycle.

## Implementation order

1. Extract a platform-neutral settings view model from the existing `SettingsEditor`.
2. Build Windows General page in WinUI 3: rail, cards, toggles, shortcut recorder, descriptions, examples, Apply/Save errors.
3. Migrate remaining Windows pages.
4. Add macOS SwiftUI Settings scene backed by the same view model.
5. Add Linux GTK4/Libadwaita preferences behind an optional feature; document KDE/Kirigami separately.
6. Retire the hand-built Win32 settings dialog after parity tests and native screenshots pass.

## Stop condition

Do not add more coordinate-based Win32 labels. The current shell is a compatibility prototype, not the target modern UI. The first WinUI General page must pass 100/150/200% DPI, keyboard-only, Narrator, light/dark, high-contrast, save-failure, and screenshot checks before more pages are migrated.
