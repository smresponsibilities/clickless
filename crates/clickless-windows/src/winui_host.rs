//! WinUI 3 bootstrap seam. The visual host is feature-gated so the existing
//! tray binary and non-Windows builds keep their current dependencies.
//!
//! The card model below is pure Rust: it binds the shared `settings_model`
//! descriptors to the `SettingsEditor` draft and is testable on every OS.
//! Only the WinRT widget construction sits behind the `winui3` feature.

use crate::settings_editor::{Fields, SettingsEditor, fields_from_config};
use clickless_config::settings_model::{self, SettingsPage};
use std::collections::BTreeMap;

/// One settings card: model metadata plus the current draft value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRow {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub value: String,
}

/// Binds one page of the shared model to the editor draft. The WinUI host
/// renders these rows; `SettingsEditor` stays the only write path.
pub fn card_rows(editor: &SettingsEditor, page: SettingsPage) -> Vec<CardRow> {
    let fields = fields_from_config(editor.draft());
    settings_model::settings_for(page)
        .map(|descriptor| CardRow {
            key: descriptor.key,
            title: descriptor.title,
            description: descriptor.description,
            value: draft_value(&fields, descriptor.key),
        })
        .collect()
}

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.to_string()
}

/// The draft value for a model key. An unknown key yields an empty string so
/// the coverage test fails instead of rendering a stale example.
fn draft_value(fields: &Fields, key: &str) -> String {
    match key {
        "enabled" => on_off(fields.enabled),
        "scroll_step" => fields.scroll_step.clone(),
        "nested_keys" => fields.grid_keys.join(" "),
        "column_keys" => fields.column_keys.join(" "),
        "row_keys" => fields.row_keys.join(" "),
        "nudge_step" => fields.nudge_step_px.clone(),
        "drag_after_select" => on_off(fields.drag_after_select),
        "auto_free_mode" => on_off(fields.auto_free_mode),
        "panel_opacity" => fields.panel_opacity.clone(),
        "color_border" => fields.border.clone(),
        "border_px" => fields.border_px.clone(),
        "color_highlight" => fields.highlight.clone(),
        "highlight_opacity" => fields.highlight_opacity.clone(),
        "color_pointer" => fields.pointer.clone(),
        "label_size" => fields.label_size.clone(),

        "leader" => fields.leader.clone(),
        "hold_ms" => fields.hold_ms.clone(),
        "start_speed" => fields.start_speed_px_s.clone(),
        "max_speed" => fields.max_speed_px_s.clone(),
        "ramp_ms" => fields.ramp_ms.clone(),
        "layout" => fields.layout.clone(),
        "nested_size" => format!("{} x {}", fields.grid_rows, fields.grid_cols),
        "nudge_enabled" => on_off(fields.nudge_enabled),
        "mouse_bindings" => fields
            .mouse_bindings
            .iter()
            .map(|(key, action)| format!("{key} = {action}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "color_panel" => fields.panel.clone(),
        "color_label" => fields.label.clone(),
        "about" => "Clickless v0.1.0".to_string(),
        _ => String::new(),
    }
}
/// One live card control, as read back from the WinUI host. The host folds a
/// map of these into the draft through `fields_with_draft`, so the write-back
/// path is testable without WinUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftValue {
    Flag(bool),
    Text(String),
    /// Index into `CHOICE_OPTIONS`.
    Choice(usize),
}

/// Labels and draft values behind every `SettingKind::Choice` control, in
/// display order. Index 0 is the `dense` layout that `draft_value` shows.
pub const CHOICE_OPTIONS: [(&str, &str); 2] = [("Dense", "dense"), ("Simple", "simple")];

/// Sidebar selection to page. An out-of-range index falls back to General, so
/// a programmatic selection change can never blank the content area.
pub fn page_for_index(index: i32) -> SettingsPage {
    SettingsPage::ALL
        .get(usize::try_from(index).unwrap_or(0))
        .copied()
        .unwrap_or(SettingsPage::General)
}

/// Folds live card values into a draft. `base` carries every field the host
/// has no control for yet, so unsupported settings keep their saved value.
/// A malformed value names its field and leaves the draft untouched, exactly
/// like `SettingsEditor::edit`.
pub fn fields_with_draft(
    base: &Fields,
    values: &BTreeMap<&'static str, DraftValue>,
) -> Result<Fields, String> {
    let mut fields = base.clone();
    for (key, value) in values {
        let key: &str = key;
        match (key, value) {
            ("enabled", DraftValue::Flag(on)) => fields.enabled = *on,
            ("scroll_step", DraftValue::Text(text)) => fields.scroll_step = text.trim().to_string(),
            ("nested_keys", DraftValue::Text(text)) => {
                fields.grid_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("column_keys", DraftValue::Text(text)) => {
                fields.column_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("row_keys", DraftValue::Text(text)) => {
                fields.row_keys = text.split_whitespace().map(str::to_string).collect()
            }
            ("nudge_step", DraftValue::Text(text)) => {
                fields.nudge_step_px = text.trim().to_string()
            }
            ("drag_after_select", DraftValue::Flag(on)) => fields.drag_after_select = *on,
            ("auto_free_mode", DraftValue::Flag(on)) => fields.auto_free_mode = *on,
            ("panel_opacity", DraftValue::Text(text)) => {
                fields.panel_opacity = text.trim().to_string()
            }
            ("color_border", DraftValue::Text(text)) => fields.border = text.trim().to_string(),
            ("border_px", DraftValue::Text(text)) => fields.border_px = text.trim().to_string(),
            ("color_highlight", DraftValue::Text(text)) => {
                fields.highlight = text.trim().to_string()
            }
            ("highlight_opacity", DraftValue::Text(text)) => {
                fields.highlight_opacity = text.trim().to_string()
            }
            ("color_pointer", DraftValue::Text(text)) => fields.pointer = text.trim().to_string(),
            ("label_size", DraftValue::Text(text)) => fields.label_size = text.trim().to_string(),

            ("nudge_enabled", DraftValue::Flag(on)) => fields.nudge_enabled = *on,
            ("leader", DraftValue::Text(text)) => fields.leader = text.trim().to_string(),
            ("hold_ms", DraftValue::Text(text)) => fields.hold_ms = text.trim().to_string(),
            ("start_speed", DraftValue::Text(text)) => {
                fields.start_speed_px_s = text.trim().to_string()
            }
            ("max_speed", DraftValue::Text(text)) => {
                fields.max_speed_px_s = text.trim().to_string()
            }
            ("ramp_ms", DraftValue::Text(text)) => fields.ramp_ms = text.trim().to_string(),
            ("color_panel", DraftValue::Text(text)) => fields.panel = text.trim().to_string(),
            ("color_label", DraftValue::Text(text)) => fields.label = text.trim().to_string(),
            ("mouse_bindings", DraftValue::Text(text)) => {
                fields.mouse_bindings = text
                    .split(['\r', '\n'])
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        line.split_once('=')
                            .map(|(key, action)| {
                                (key.trim().to_string(), action.trim().to_string())
                            })
                            .ok_or_else(|| format!("Shortcut must use key = action: {line}"))
                    })
                    .collect::<Result<_, _>>()?;
            }
            ("nested_size", DraftValue::Text(text)) => {
                let (rows, cols) = split_subgrid(text)?;
                fields.grid_rows = rows;
                fields.grid_cols = cols;
            }
            ("layout", DraftValue::Choice(index)) => {
                fields.layout = CHOICE_OPTIONS
                    .get(*index)
                    .ok_or_else(|| format!("Grid style: no option {index}"))?
                    .1
                    .to_string();
            }
            _ => {}
        }
    }
    Ok(fields)
}

/// Splits the `"3 x 10"` subgrid label that `draft_value` renders.
fn split_subgrid(text: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = text.split(['x', '×']).map(str::trim).collect();
    match parts.as_slice() {
        [rows, cols] if !rows.is_empty() && !cols.is_empty() => {
            Ok((rows.to_string(), cols.to_string()))
        }
        _ => Err(format!("Subgrid size: write \"3 x 10\", not \"{text}\"")),
    }
}

#[cfg(feature = "winui3")]
pub mod enabled {
    //! WinUI 3 settings host. It is created on the runtime loop thread and
    //! never starts a loop of its own: the existing message pump dispatches
    //! the WinUI messages and the dispatcher queue work. A failure before the
    //! window is up returns `Err`, so the caller falls back to the native
    //! Win32 shell instead of leaving the owner without settings.

    use super::{
        CHOICE_OPTIONS, DraftValue, draft_value, fields_from_config, fields_with_draft,
        page_for_index,
    };
    use crate::settings_editor::{Fields, SettingsEditor};
    use clickless_config::{Config, SettingDescriptor, SettingKind, SettingsPage, settings_model};
    use std::collections::BTreeMap;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex, Once, OnceLock};

    use ::winui3 as bootstrap_winui3;
    use bootstrap_winui3::bootstrap::{PackageDependency, WindowsAppSDKVersion};
    use bootstrap_winui3::{
        ApartmentType, ChildClass, ChildClassImpl, Compose, CreateInstanceFn, IWindowNative,
    };
    use naui_winui3 as winui3;
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::UI::Text::FontWeights;
    use windows_core::{
        Array, HSTRING, IInspectable_Vtbl, Interface, Ref, imp::WeakRefCount, implement,
    };
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, ICON_BIG, ICON_SMALL, IMAGE_ICON, IsWindowVisible,
        LR_DEFAULTSIZE, LR_SHARED, LoadImageW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW,
        SetForegroundWindow, SetWindowPos, WM_SETICON,
    };
    use winui3::Microsoft::UI::Dispatching::{DispatcherQueue, DispatcherQueueHandler};
    use winui3::Microsoft::UI::Xaml::Controls::{
        AutoSuggestBox, Button, ColorPicker, ComboBox, ComboBoxItem, Control, Expander, Grid,
        InfoBar, InfoBarSeverity, NavigationView, NumberBox, NumberBoxValidationMode, Orientation,
        StackPanel, TextBlock, TextBox, ToggleSwitch, XamlControlsResources,
    };
    use winui3::Microsoft::UI::Xaml::Markup::{
        IXamlMetadataProvider, IXamlMetadataProvider_Impl, IXamlType, XmlnsDefinition,
    };
    use winui3::Microsoft::UI::Xaml::XamlTypeInfo::XamlControlsXamlMetaDataProvider;
    use winui3::Microsoft::UI::Xaml::{
        Application, ApplicationInitializationCallback, IApplicationFactory,
        IApplicationFactory_Vtbl, IApplicationOverrides, IApplicationOverrides_Impl,
        LaunchActivatedEventArgs, ResourceDictionary, RoutedEventHandler, TextWrapping, Thickness,
        Window,
    };
    use winui3::Windows::UI::Xaml::Interop::TypeName;

    /// Settings window design size at 96 dpi, matching the native shell.
    const SETTINGS_WIDTH: i32 = 960;
    const SETTINGS_HEIGHT: i32 = 760;

    const LOCK_FAILED: &str = "settings state was poisoned";

    /// Loads the Clickless app icon from the embedded resource for the Settings window.
    fn load_app_icon() -> Result<HWND, String> {
        let icon = unsafe {
            LoadImageW(
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                101 as *const _, // Use resource ID 101 directly
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE | LR_SHARED,
            )
        };
        if icon.is_null() {
            return Err("failed to load Clickless app icon from resource 101".to_string());
        }
        Ok(icon)
    }

    /// Sets the Clickless app icon on the Settings window for taskbar and Alt-Tab.
    fn set_window_icon(hwnd: HWND, icon: HWND) {
        // windows-sys 0.61 has no `SetIcon` wrapper, so send the same
        // WM_SETICON messages user32's SetIcon issues internally.
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
        }
    }

    /// Runtime push used by Apply and Save. It runs on the loop thread inside a
    /// Runtime push used by Apply and Save. It runs on the loop thread inside a
    /// WinRT event, so it reaches the hook the way the keyboard callback does.
    /// `Send` because the WinRT delegates require it.
    pub type ApplyCallback = Box<dyn FnMut(&Config) -> Result<(), String> + Send>;

    /// The XAML dispatcher queue, owned by a dedicated UI thread. `Application::Start`
    /// runs the WinUI message pump internally and blocks until shutdown, so the
    /// whole XAML runtime lives on its own STA thread; the loop thread only talks
    /// to it through this queue. Filled once, with either the queue or the reason
    /// the runtime could not start.
    static XAML_DISPATCHER: OnceLock<Result<DispatcherQueue, String>> = OnceLock::new();
    static XAML_SPAWN: Once = Once::new();

    /// Starts the XAML thread on first use and waits briefly for its dispatcher queue.
    /// A bare `Application::new()` plus a manually created dispatcher queue is
    /// not a valid WinUI 3 host: `Window::new` fails with RPC_E_WRONG_THREAD
    /// (0x8001010E). The working sequence is STA apartment, bootstrap
    /// dependency, then `Application::Start` with a composed `Application`
    /// subclass carrying a XAML metadata provider.
    pub fn ensure_xaml_thread() -> Result<DispatcherQueue, String> {
        XAML_SPAWN.call_once(|| {
            let spawned = std::thread::Builder::new()
                .name("clickless-xaml".to_string())
                .spawn(xaml_thread_main);
            if let Err(error) = spawned {
                let _ = XAML_DISPATCHER.set(Err(format!("XAML thread spawn failed: {error}")));
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(result) = XAML_DISPATCHER.get() {
                return result.clone();
            }
            if std::time::Instant::now() >= deadline {
                return Err("WinUI settings startup timed out".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// XAML thread body. Everything XAML happens here: apartment, bootstrap,
    /// `Application::Start` (which pumps until the process exits).
    pub(crate) fn xaml_thread_main() {
        let dependency = match bootstrap() {
            Ok(dependency) => dependency,
            Err(error) => {
                let _ = XAML_DISPATCHER.set(Err(error));
                return;
            }
        };
        // The bootstrap dependency must stay loaded for the pump's lifetime;
        // `PackageDependency` is not `Send`, so leak it to the process instead
        // of capturing it in the `Send` callback.
        std::mem::forget(dependency);
        let callback = ApplicationInitializationCallback::new(move |_| {
            let result = start_xaml_app().map_err(|error| error.to_string());
            let _ = XAML_DISPATCHER.set(result);
            Ok(())
        });
        if let Err(error) = Application::Start(&callback) {
            let _ = XAML_DISPATCHER.set(Err(format!("XAML application exited: {error}")));
        }
    }

    fn bootstrap() -> Result<PackageDependency, String> {
        bootstrap_winui3::init_apartment(ApartmentType::SingleThreaded)
            .map_err(|error| format!("XAML thread apartment init failed: {error}"))?;
        PackageDependency::initialize_version(WindowsAppSDKVersion::V2)
            .map_err(|error| format!("Windows App SDK runtime unavailable: {error}"))
    }

    /// Runs inside the `Application::Start` callback, on the XAML thread with
    /// the framework fully initialized.
    fn start_xaml_app() -> windows_core::Result<DispatcherQueue> {
        let app = XamlApp::compose()?;

        // The application object must outlive the pump; the callback scope does
        // not, so hand ownership to the process.
        std::mem::forget(app);
        DispatcherQueue::GetForCurrentThread()
    }

    /// Composed `Application` subclass: a XAML metadata provider lets the
    /// framework resolve control types, and the merged dictionaries style them.
    #[implement(IApplicationOverrides, IXamlMetadataProvider)]
    struct XamlApp {
        provider: XamlControlsXamlMetaDataProvider,
    }

    impl XamlApp {
        fn compose() -> windows_core::Result<Application> {
            Compose::compose(Self {
                provider: XamlControlsXamlMetaDataProvider::new()?,
            })
        }
    }

    impl ChildClassImpl for XamlApp_Impl {}

    impl IApplicationOverrides_Impl for XamlApp_Impl {
        fn OnLaunched(&self, _: Ref<LaunchActivatedEventArgs>) -> windows_core::Result<()> {
            let resources = self.base()?.cast::<Application>()?.Resources()?;
            let merged_dictionaries = resources.MergedDictionaries()?;
            merged_dictionaries.Append(&XamlControlsResources::new()?)?;
            let compact = ResourceDictionary::new()?;
            compact.SetSource(&Uri::CreateUri(&HSTRING::from(
                "ms-appx:///Microsoft.UI.Xaml/DensityStyles/Compact.xaml",
            ))?)?;
            merged_dictionaries.Append(&compact)?;
            Ok(())
        }
    }

    impl IXamlMetadataProvider_Impl for XamlApp_Impl {
        fn GetXamlType(&self, ty: &TypeName) -> windows_core::Result<IXamlType> {
            self.provider.GetXamlType(ty)
        }

        fn GetXamlTypeByFullName(&self, name: &HSTRING) -> windows_core::Result<IXamlType> {
            self.provider.GetXamlTypeByFullName(name)
        }

        fn GetXmlnsDefinitions(&self) -> windows_core::Result<Array<XmlnsDefinition>> {
            self.provider.GetXmlnsDefinitions()
        }
    }

    impl ChildClass for XamlApp {
        type BaseType = Application;
        type FactoryInterface = IApplicationFactory;

        fn create_interface_fn(vtable: &IApplicationFactory_Vtbl) -> CreateInstanceFn {
            vtable.CreateInstance
        }

        fn identity_vtable(vtable: &mut Self::Outer) -> &mut &'static IInspectable_Vtbl {
            &mut vtable.identity
        }

        fn ref_count(vtable: &Self::Outer) -> &WeakRefCount {
            &vtable.count
        }

        fn into_outer(self) -> Self::Outer {
            Self::into_outer(self)
        }
    }

    /// Draft, rendered cards and footer state shared with the WinRT event
    /// handlers, which must be `Send + 'static`.
    enum FieldControl {
        Widget(Control),
        Bindings(crate::winui_bindings::BindingEditor),
        Subgrid(NumberBox, NumberBox),
    }

    struct Shared {
        editor: Mutex<SettingsEditor>,
        cards: Mutex<StackPanel>,
        values: Mutex<BTreeMap<&'static str, DraftValue>>,
        /// Live control per model key for the visible page.
        controls: Mutex<BTreeMap<&'static str, FieldControl>>,
        sidebar: NavigationView,
        search: AutoSuggestBox,
        status: InfoBar,
        heading: TextBlock,
        errors: Mutex<BTreeMap<&'static str, TextBlock>>,
        advanced: Mutex<Option<Expander>>,
        apply: Mutex<ApplyCallback>,
    }

    impl Shared {
        /// Reads every live card back into the draft. A malformed value names
        /// its field and leaves the draft untouched, like the native shell.
        fn sync_draft(&self) -> Result<(), String> {
            self.capture_controls()?;
            for control in self.controls.lock().map_err(|_| LOCK_FAILED)?.values() {
                if let FieldControl::Bindings(editor) = control {
                    editor.validate()?;
                }
            }
            let values = self.values.lock().map_err(|_| LOCK_FAILED)?.clone();
            let mut editor = self.editor.lock().map_err(|_| LOCK_FAILED)?;
            let base = fields_from_config(editor.draft());
            let fields = fields_with_draft(&base, &values)?;
            editor.edit(&fields)
        }

        fn capture_controls(&self) -> Result<(), String> {
            self.values
                .lock()
                .map_err(|_| LOCK_FAILED)?
                .extend(self.read_controls());
            Ok(())
        }

        fn read_controls(&self) -> BTreeMap<&'static str, DraftValue> {
            let Ok(controls) = self.controls.lock() else {
                return BTreeMap::new();
            };
            controls
                .iter()
                .filter_map(|(key, control)| read_control(key, control).map(|value| (*key, value)))
                .collect()
        }

        /// Apply: validate the draft, then push it to the running engine.
        fn apply(&self) -> Result<(), String> {
            self.sync_draft()?;
            let mut apply = self.apply.lock().map_err(|_| LOCK_FAILED)?;
            let mut editor = self.editor.lock().map_err(|_| LOCK_FAILED)?;
            editor.apply_to_runtime(|config| apply(config))?;
            Ok(())
        }

        /// Save: apply to the runtime, then write through the atomic save. A
        /// failed write keeps the previous file and reports the error.
        fn save(&self) -> Result<String, String> {
            if std::env::args().any(|arg| arg == "--runtime") {
                self.apply()?;
            } else {
                self.sync_draft()?;
                self.editor.lock().map_err(|_| LOCK_FAILED)?.apply()?;
            }
            let path = crate::settings_process::config_path()?;
            let editor = self.editor.lock().map_err(|_| LOCK_FAILED)?;
            editor.save(&path).map_err(|error| error.to_string())?;
            Ok(path.display().to_string())
        }

        fn reset_all(&self) -> Result<(), String> {
            self.editor.lock().map_err(|_| LOCK_FAILED)?.reset_all();
            self.values.lock().map_err(|_| LOCK_FAILED)?.clear();
            Ok(())
        }

        fn cancel(&self) -> Result<(), String> {
            self.editor.lock().map_err(|_| LOCK_FAILED)?.cancel();
            self.values.lock().map_err(|_| LOCK_FAILED)?.clear();
            Ok(())
        }

        fn set_status(&self, text: &str) {
            let message: String = text.chars().take(256).collect();
            let error = !(text.starts_with("Applied")
                || text.starts_with("Saved")
                || text.starts_with("Cancelled")
                || text.starts_with("Draft reset"));
            let _ = self.status.SetSeverity(if error {
                InfoBarSeverity::Error
            } else {
                InfoBarSeverity::Success
            });
            let _ = self.status.SetMessage(&HSTRING::from(message));
            let _ = self.status.SetIsOpen(true);
        }

        fn validation_feedback(&self, message: &str) {
            let lowered = message.to_lowercase();
            let aliases = [
                ("start_speed", "start speed"),
                ("max_speed", "max speed"),
                ("ramp_ms", "ramp"),
                ("hold_ms", "hold"),
                ("scroll_step", "scroll"),
                ("mouse_bindings", "binding"),
                ("mouse_bindings", "shortcut"),
                ("nested_size", "dimensions"),
                ("nested_keys", "grid"),
                ("nudge_step", "nudge"),
                ("panel_opacity", "panel opacity"),
                ("highlight_opacity", "highlight opacity"),
                ("border_px", "border"),
                ("label_size", "label size"),
                ("leader", "leader"),
            ];
            let mut showed = false;
            if let Ok(errors) = self.errors.lock() {
                for (&key, label) in errors.iter() {
                    let matches = aliases
                        .iter()
                        .any(|(field, phrase)| *field == key && lowered.contains(phrase))
                        || settings_model::SETTINGS
                            .iter()
                            .any(|d| d.key == key && lowered.contains(&d.title.to_lowercase()));
                    let _ = label.SetText(&HSTRING::from(if matches { message } else { "" }));
                    let _ = label.SetVisibility(if matches {
                        winui3::Microsoft::UI::Xaml::Visibility::Visible
                    } else {
                        winui3::Microsoft::UI::Xaml::Visibility::Collapsed
                    });
                    showed |= matches;
                }
            }
            if showed
                && let Ok(advanced) = self.advanced.lock()
                && let Some(group) = advanced.as_ref()
            {
                let _ = group.SetIsExpanded(true);
            }
        }

        /// Redraws the page the sidebar selects, from the current draft.
        fn render_current_page(&self) {
            let page = match self.sidebar.SelectedItem().and_then(|item| {
                let mut index = 0;
                self.sidebar.MenuItems()?.IndexOf(&item, &mut index)?;
                Ok(index as i32)
            }) {
                Ok(index) => page_for_index(index),
                Err(_) => SettingsPage::General,
            };
            self.render_page(page);
        }

        fn render_page(&self, page: SettingsPage) {
            if let Err(error) = self.render_page_inner(page) {
                self.set_status(&format!("{} page failed to draw: {error}", page.title()));
            }
        }

        fn render_page_inner(&self, page: SettingsPage) -> Result<(), String> {
            let fields = fields_from_config(self.editor.lock().map_err(|_| LOCK_FAILED)?.draft());
            let mut controls = self.controls.lock().map_err(|_| LOCK_FAILED)?;
            controls.clear();
            let cards = self.cards.lock().map_err(|_| LOCK_FAILED)?;
            cards
                .Children()
                .map_err(|error| error.to_string())?
                .Clear()
                .map_err(|error| error.to_string())?;
            let query = self
                .search
                .Text()
                .map_err(|error| error.to_string())?
                .to_string()
                .to_lowercase();
            self.heading
                .SetText(&HSTRING::from(if query.is_empty() {
                    page.title()
                } else {
                    "Search results"
                }))
                .map_err(|e| e.to_string())?;
            self.errors.lock().map_err(|_| LOCK_FAILED)?.clear();
            let values = self.values.lock().map_err(|_| LOCK_FAILED)?;
            let advanced_cards: StackPanel = xaml(r#"<StackPanel Spacing="8" />"#)?;
            let advanced: Expander = xaml(r#"<Expander HorizontalAlignment="Stretch" />"#)?;
            advanced
                .SetHeader(&text_block(
                    if page == SettingsPage::Appearance {
                        "Advanced appearance"
                    } else {
                        "Advanced targeting"
                    },
                    14.0,
                )?)
                .map_err(|e| e.to_string())?;
            advanced
                .SetContent(&advanced_cards)
                .map_err(|e| e.to_string())?;
            advanced.SetIsExpanded(true).map_err(|e| e.to_string())?;
            let mut advanced_count = 0;
            *self.advanced.lock().map_err(|_| LOCK_FAILED)? = Some(advanced.clone());
            for descriptor in settings_model::SETTINGS.iter().filter(|descriptor| {
                if query.is_empty() {
                    descriptor.page == page
                } else {
                    format!(
                        "{} {} {}",
                        descriptor.title,
                        descriptor.description,
                        descriptor.page.title()
                    )
                    .to_lowercase()
                    .contains(&query)
                }
            }) {
                let (card, control, error) = make_card(descriptor, &fields)?;
                self.errors
                    .lock()
                    .map_err(|_| LOCK_FAILED)?
                    .insert(descriptor.key, error);
                if let Some(value) = values.get(descriptor.key) {
                    set_control(&control, value)?;
                }
                controls.insert(descriptor.key, control);
                let advanced_field = query.is_empty()
                    && ((page == SettingsPage::Grid
                        && !["layout", "nested_size", "nudge_enabled"].contains(&descriptor.key))
                        || (page == SettingsPage::Appearance
                            && !["color_panel", "color_label"].contains(&descriptor.key)));
                if advanced_field {
                    advanced_cards
                        .Children()
                        .map_err(|e| e.to_string())?
                        .Append(&card)
                        .map_err(|e| e.to_string())?;
                    advanced_count += 1;
                } else {
                    cards
                        .Children()
                        .map_err(|e| e.to_string())?
                        .Append(&card)
                        .map_err(|e| e.to_string())?;
                }
            }
            if advanced_count > 0 {
                cards
                    .Children()
                    .map_err(|e| e.to_string())?
                    .Append(&advanced)
                    .map_err(|e| e.to_string())?;
            }
            if controls.is_empty() {
                cards
                    .Children()
                    .map_err(|error| error.to_string())?
                    .Append(&text_block(
                        "No matching settings. Try activation, speed, grid, or color.",
                        14.0,
                    )?)
                    .map_err(|error| error.to_string())?;
            }
            if query.is_empty() && page == SettingsPage::General {
                cards
                    .Children()
                    .map_err(|error| error.to_string())?
                    .Append(&built_in_shortcuts_card()?)
                    .map_err(|error| error.to_string())?;
            }
            if query.is_empty() && page == SettingsPage::About {
                cards
                    .Children()
                    .map_err(|error| error.to_string())?
                    .Append(&practice_card()?)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    }

    /// Live WinUI settings window. One per process; the window and every XAML
    /// call live on the dedicated XAML thread, reached through `dispatcher`.
    pub struct WinUiSettings {
        window: Window,
        hwnd: HWND,
        shared: Arc<Shared>,
        dispatcher: DispatcherQueue,
    }

    impl WinUiSettings {
        /// Builds the sidebar, cards and footer on the XAML thread, then shows
        /// the window. `seed` is the running config; `on_apply` pushes an
        /// accepted draft into the engine. Blocks until the XAML thread reports
        /// the built window or an error.
        pub fn create(seed: Config, on_apply: ApplyCallback) -> Result<Self, String> {
            let dispatcher = ensure_xaml_thread()?;
            let (tx, rx) = mpsc::channel();
            // The handler is `Fn`, so the moved-out arguments sit behind locks.
            let seed = Mutex::new(Some(seed));
            let on_apply = Mutex::new(Some(on_apply));
            let handler = DispatcherQueueHandler::new(move || {
                let seed = seed.lock().map(|mut slot| slot.take());
                let on_apply = on_apply.lock().map(|mut slot| slot.take());
                let result = match (seed, on_apply) {
                    (Ok(Some(seed)), Ok(Some(on_apply))) => build_window(seed, on_apply),
                    _ => Err("settings state was poisoned".to_string()),
                };
                let _ = tx.send(result);
                Ok(())
            });
            dispatcher
                .TryEnqueue(&handler)
                .map_err(|error| format!("settings build enqueue failed: {error}"))?;
            let (window, hwnd, shared) = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|error| format!("settings build timed out: {error}"))??;
            // `HWND` is a raw pointer and not `Send`; it crossed the channel as
            // `usize` and is only used with thread-agnostic Win32 calls.
            let hwnd = hwnd as HWND;
            Ok(Self {
                window,
                hwnd,
                shared,
                dispatcher,
            })
        }
    }

    /// Runs on the XAML thread via `DispatcherQueue::TryEnqueue`; every XAML
    /// object created here is thread-local to it.
    fn build_window(
        seed: Config,
        on_apply: ApplyCallback,
    ) -> Result<(Window, usize, Arc<Shared>), String> {
        let window = Window::new().map_err(|error| format!("WinUI window: {error}"))?;
        let hwnd = window_handle(&window)?;
        // Set the Clickless app icon on the window
        if let Ok(icon) = load_app_icon() {
            set_window_icon(hwnd, icon);
        }
        resize(hwnd, SETTINGS_WIDTH, SETTINGS_HEIGHT)?;
        crate::window_style::dark_caption(hwnd);
        window
            .SetTitle(&HSTRING::from("Clickless Settings"))
            .map_err(|error| error.to_string())?;

        let root: Grid = winui3::Microsoft::UI::Xaml::Markup::XamlReader::Load(&HSTRING::from(
            include_str!("settings_shell.xaml"),
        ))
        .and_then(|v| v.cast())
        .map_err(|e| e.to_string())?;
        crate::winui_windows::attach_theme(&root)?;
        let find = |name: &str| {
            root.FindName(&HSTRING::from(name))
                .map_err(|e| e.to_string())
        };
        let sidebar = find("Navigation")?
            .cast::<NavigationView>()
            .map_err(|e| e.to_string())?;
        let search = find("Search")?
            .cast::<AutoSuggestBox>()
            .map_err(|e| e.to_string())?;
        let cards = find("Cards")?
            .cast::<StackPanel>()
            .map_err(|e| e.to_string())?;
        let status = find("Status")?
            .cast::<InfoBar>()
            .map_err(|e| e.to_string())?;
        let heading = find("Heading")?
            .cast::<TextBlock>()
            .map_err(|e| e.to_string())?;
        sidebar
            .SetSelectedItem(
                &sidebar
                    .MenuItems()
                    .map_err(|e| e.to_string())?
                    .GetAt(0)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        let shared = Arc::new(Shared {
            editor: Mutex::new(SettingsEditor::new(seed)),
            cards: Mutex::new(cards),
            controls: Mutex::new(BTreeMap::new()),
            values: Mutex::new(BTreeMap::new()),
            sidebar: sidebar.clone(),
            search: search.clone(),
            status,
            heading,
            errors: Mutex::new(BTreeMap::new()),
            advanced: Mutex::new(None),
            apply: Mutex::new(on_apply),
        });
        find("Actions")?
            .cast::<Grid>()
            .map_err(|e| e.to_string())?
            .Children()
            .map_err(|e| e.to_string())?
            .Append(&footer(&shared)?)
            .map_err(|e| e.to_string())?;
        {
            let shared = Arc::clone(&shared);
            sidebar
                .SelectionChanged(&TypedEventHandler::new(move |_, _| {
                    if let Err(error) = shared.capture_controls() {
                        shared.set_status(&error);
                    }
                    shared.render_current_page();
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
        }
        {
            let timer = ensure_xaml_thread()?
                .CreateTimer()
                .map_err(|e| e.to_string())?;
            timer
                .SetInterval(windows::Foundation::TimeSpan {
                    Duration: 5_000_000,
                })
                .map_err(|e| e.to_string())?;
            timer.SetIsRepeating(false).map_err(|e| e.to_string())?;
            let target = Arc::clone(&shared);
            timer
                .Tick(&TypedEventHandler::new(move |_, _| {
                    if let Err(error) = target.capture_controls() {
                        target.set_status(&error);
                        return Ok(());
                    }
                    target.render_current_page();
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
            let target = Arc::clone(&shared);
            search
                .TextChanged(&TypedEventHandler::<
                    AutoSuggestBox,
                    winui3::Microsoft::UI::Xaml::Controls::AutoSuggestBoxTextChangedEventArgs,
                >::new(move |sender, args| {
                    if let Some(args) = args.as_ref()
                        && args.Reason()? != winui3::Microsoft::UI::Xaml::Controls::AutoSuggestionBoxTextChangeReason::UserInput
                    {
                        timer.Stop()?;
                        return Ok(());
                    }
                    if let Err(error) = target.capture_controls() {
                        target.set_status(&error);
                    }
                    if let Some(sender) = sender.as_ref() {
                        let query = sender.Text()?.to_string().to_lowercase();
                        let suggestions = settings_model::SETTINGS
                            .iter()
                            .filter(|d| {
                                !query.is_empty()
                                    && format!("{} {} {}", d.title, d.description, d.page.title())
                                        .to_lowercase()
                                        .contains(&query)
                            })
                            .take(8)
                            .map(|d| {
                                windows::Foundation::PropertyValue::CreateString(&HSTRING::from(
                                    d.title,
                                ))
                                .map(Some)
                            })
                            .collect::<windows_core::Result<Vec<_>>>()?;
                        let items =
                            windows_collections::IVectorView::<windows_core::IInspectable>::from(
                                suggestions,
                            );
                        sender.SetItemsSource(&items)?;
                    }
                    timer.Stop()?;
                    timer.Start()?;
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
            let target = Arc::clone(&shared);
            search
                .QuerySubmitted(&TypedEventHandler::<
                    AutoSuggestBox,
                    winui3::Microsoft::UI::Xaml::Controls::AutoSuggestBoxQuerySubmittedEventArgs,
                >::new(move |sender, args| {
                    if let (Some(sender), Some(args)) = (sender.as_ref(), args.as_ref()) {
                        let title = args.QueryText()?.to_string();
                        if let Some(descriptor) = settings_model::SETTINGS
                            .iter()
                            .find(|d| d.title.eq_ignore_ascii_case(&title))
                        {
                            let index = SettingsPage::ALL
                                .iter()
                                .position(|p| *p == descriptor.page)
                                .unwrap_or(0);
                            target.sidebar.SetSelectedItem(
                                &target.sidebar.MenuItems()?.GetAt(index as u32)?,
                            )?;
                            sender.SetText(&HSTRING::new())?;
                            target.render_page(descriptor.page);
                            if let Ok(group) = target.advanced.lock()
                                && let Some(group) = group.as_ref()
                            {
                                group.SetIsExpanded(true)?;
                            }
                            if let Ok(controls) = target.controls.lock()
                                && let Some(FieldControl::Widget(control)) =
                                    controls.get(descriptor.key)
                            {
                                let _ = crate::winui_windows::focus_element(control);
                            }
                        } else {
                            target.render_current_page();
                        }
                    }
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
        }
        {
            let search = search.clone();
            root.PreviewKeyDown(&winui3::Microsoft::UI::Xaml::Input::KeyEventHandler::new(
                move |_, args| {
                    if let Some(args) = args.as_ref()
                        && args.Key()?.0 == 70
                        && unsafe {
                            windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x11)
                        } < 0
                    {
                        crate::winui_windows::focus_element(&search)
                            .map_err(|_| windows_core::Error::empty())?;
                        args.SetHandled(true)?;
                    }
                    Ok(())
                },
            ))
            .map_err(|e| e.to_string())?;
        }
        crate::winui_windows::set_window_content(&window, &root, "Clickless Settings")?;
        window.Activate().map_err(|error| error.to_string())?;
        shared.render_page(SettingsPage::General);

        Ok((window, hwnd as usize, shared))
    }

    impl WinUiSettings {
        /// Raises the window. Reopening from the tray keeps the same window and
        /// puts it in front instead of leaving it behind other apps. XAML calls
        /// are marshalled to the XAML thread through its dispatcher queue.
        pub fn show(&self) -> Result<(), String> {
            if !self.is_visible() {
                return self.activate();
            }
            unsafe { BringWindowToTop(self.hwnd) };
            self.activate()?;
            unsafe { SetForegroundWindow(self.hwnd) };
            Ok(())
        }

        /// `Window` methods must run on the thread that created the window, so
        /// `Activate` is enqueued on the XAML thread and awaited here.
        fn activate(&self) -> Result<(), String> {
            let window = self.window.clone();
            let (tx, rx) = mpsc::channel();
            let handler = DispatcherQueueHandler::new(move || {
                let _ = tx.send(window.Activate().map_err(|error| error.to_string()));
                Ok(())
            });
            self.dispatcher
                .TryEnqueue(&handler)
                .map_err(|error| format!("settings raise enqueue failed: {error}"))?;
            rx.recv().map_err(|_| "XAML thread exited".to_string())?
        }

        /// Focus check for the loop: while the settings window owns the
        /// foreground, the hook passes keys through instead of consuming them.
        pub fn has_focus(&self) -> bool {
            unsafe { GetForegroundWindow() == self.hwnd }
        }

        pub fn is_visible(&self) -> bool {
            unsafe { IsWindowVisible(self.hwnd) != 0 }
        }

        /// Shows a pending startup notice once, in the status line.
        pub fn notice(&self) {
            if let Some(notice) = crate::settings::win::take_notice() {
                self.shared.set_status(&notice);
            }
        }

        /// Executes a closure on the XAML thread via the dispatcher queue synchronously.
        /// This ensures WinUI object access only happens on the XAML thread, preventing
        /// cross-thread access from the hook loop.
        fn request_sync<F, T>(&self, f: F) -> Result<T, String>
        where
            F: FnOnce() -> Result<T, String> + Send + 'static,
            T: Send + 'static,
        {
            let (tx, rx) = mpsc::channel();
            // Wrap f in Mutex so the Fn closure can take it once
            let opt_f = std::sync::Mutex::new(Some(f));
            let handler = DispatcherQueueHandler::new(move || {
                if let Some(f) = opt_f.lock().unwrap().take() {
                    let result = f();
                    let _ = tx.send(result);
                }
                Ok(())
            });
            self.dispatcher
                .TryEnqueue(&handler)
                .map_err(|error| format!("request enqueue failed: {error}"))?;
            rx.recv().map_err(|_| "XAML thread exited".to_string())?
        }

        /// Requests whether the settings window has focus, executing on the XAML thread.
        pub fn request_has_focus(&self) -> Result<bool, String> {
            let hwnd = self.hwnd as usize;
            self.request_sync(move || Ok(unsafe { GetForegroundWindow() as usize == hwnd }))
        }

        /// Requests whether the settings window is visible, executing on the XAML thread.
        pub fn request_is_visible(&self) -> Result<bool, String> {
            let hwnd = self.hwnd as usize;
            self.request_sync(move || Ok(unsafe { IsWindowVisible(hwnd as *mut _) != 0 }))
        }
    }

    impl Drop for WinUiSettings {
        /// Closes the window on the XAML thread when the host is dropped.
        fn drop(&mut self) {
            let window = self.window.clone();
            let handler = DispatcherQueueHandler::new(move || {
                let _ = window.Close();
                Ok(())
            });
            let _ = self.dispatcher.TryEnqueue(&handler);
        }
    }

    /// A wrapped text block sized for its role in the card.
    pub(crate) fn xaml<T: Interface>(markup: &str) -> Result<T, String> {
        let markup = markup.replacen(
            " ",
            " xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" ",
            1,
        );
        winui3::Microsoft::UI::Xaml::Markup::XamlReader::Load(&HSTRING::from(markup))
            .and_then(|value| value.cast())
            .map_err(|e| e.to_string())
    }

    fn named_control<T: Interface>(kind: &str, title: &str) -> Result<T, String> {
        let title = title
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;");
        xaml(&format!("<{kind} AutomationProperties.Name=\"{title}\" />"))
    }

    pub(crate) fn text_block(text: &str, size: f64) -> Result<TextBlock, String> {
        let block = TextBlock::new().map_err(|error| error.to_string())?;
        block
            .SetText(&HSTRING::from(text))
            .map_err(|error| error.to_string())?;
        block.SetFontSize(size).map_err(|error| error.to_string())?;
        block
            .SetTextWrapping(TextWrapping::Wrap)
            .map_err(|error| error.to_string())?;
        Ok(block)
    }

    pub(crate) fn labeled_button(label: &str) -> Result<Button, String> {
        let button = Button::new().map_err(|error| error.to_string())?;
        button
            .SetContent(&text_block(label, 14.0)?)
            .map_err(|error| error.to_string())?;
        Ok(button)
    }

    /// Native top-level handle, used for focus detection and placement.
    pub(crate) fn window_handle(window: &Window) -> Result<HWND, String> {
        let native = Interface::cast::<IWindowNative>(window).map_err(|error| error.to_string())?;
        let handle = unsafe { native.WindowHandle() }.map_err(|error| error.to_string())?;
        if handle.0.is_null() {
            return Err("WinUI window reported no native handle".to_string());
        }
        Ok(handle.0 as HWND)
    }

    /// Design size scaled by the system dpi, like the native shell.
    pub(crate) fn resize(hwnd: HWND, design_width: i32, design_height: i32) -> Result<(), String> {
        let dpi = unsafe { GetDpiForSystem() } as i32;
        let mut area = windows_sys::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: SETTINGS_WIDTH,
            bottom: SETTINGS_HEIGHT,
        };
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
                windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA,
                0,
                &mut area as *mut _ as _,
                0,
            );
        }
        let width = (design_width * dpi / 96).min(area.right - area.left);
        let height = (design_height * dpi / 96).min(area.bottom - area.top);
        let applied = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                area.left + (area.right - area.left - width) / 2,
                area.top + (area.bottom - area.top - height) / 2,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if applied == 0 {
            return Err("SetWindowPos failed for the WinUI settings window".to_string());
        }
        Ok(())
    }

    /// A button that runs one draft action and reports the outcome in the
    /// status line, redrawing the page so the cards show what was kept.
    fn action_button(
        label: &str,
        shared: &Arc<Shared>,
        action: impl Fn(&Shared) -> Result<String, String> + Send + 'static,
    ) -> Result<Button, String> {
        let button = labeled_button(label)?;
        let target = Arc::clone(shared);
        let handler = RoutedEventHandler::new(move |_sender, _args| {
            let message = match action(&target) {
                Ok(message) => {
                    if let Ok(mut values) = target.values.lock() {
                        values.clear();
                    }
                    target.render_current_page();
                    message
                }
                Err(error) => {
                    target.validation_feedback(&error);
                    error
                }
            };
            target.set_status(&message);
            Ok(())
        });
        button.Click(&handler).map_err(|error| error.to_string())?;
        Ok(button)
    }

    /// Footer: the write path. Apply pushes the draft to the engine, Save
    /// applies and writes the file, Cancel drops the draft, Reset all restores
    /// factory defaults in the draft only.
    fn footer(shared: &Arc<Shared>) -> Result<StackPanel, String> {
        let row = StackPanel::new().map_err(|error| error.to_string())?;
        row.SetOrientation(Orientation::Horizontal)
            .map_err(|error| error.to_string())?;
        row.SetSpacing(8.0).map_err(|error| error.to_string())?;
        row.SetMargin(Thickness {
            Left: 16.0,
            Top: 0.0,
            Right: 16.0,
            Bottom: 8.0,
        })
        .map_err(|error| error.to_string())?;

        let apply = action_button("Apply", shared, |shared| {
            shared
                .apply()
                .map(|()| "Applied to the running engine.".to_string())
        })?;
        let save = action_button("Save", shared, |shared| {
            shared.save().map(|path| format!("Saved to {path}"))
        })?;
        let accent: Button = xaml(r#"<Button Style="{StaticResource AccentButtonStyle}" />"#)?;
        save.SetStyle(&accent.Style().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let cancel = action_button("Cancel", shared, |shared| {
            shared
                .cancel()
                .map(|()| "Cancelled; the cards show the applied settings again.".to_string())
        })?;
        let reset = action_button("Reset all", shared, |shared| {
            shared
                .reset_all()
                .map(|()| "Draft reset to factory defaults; Apply or Save to keep it.".to_string())
        })?;

        for button in [&apply, &save, &cancel, &reset] {
            row.Children()
                .map_err(|error| error.to_string())?
                .Append(button)
                .map_err(|error| error.to_string())?;
        }
        Ok(row)
    }

    /// About page: the practice entry. The walkthrough itself stays in
    /// `practice_dialog`, the keyboard-driven flow over the real engine, so the
    /// new shell does not grow a second copy of it.
    fn practice_card() -> Result<StackPanel, String> {
        let card = StackPanel::new().map_err(|error| error.to_string())?;
        card.SetOrientation(Orientation::Vertical)
            .map_err(|error| error.to_string())?;
        card.SetSpacing(6.0).map_err(|error| error.to_string())?;
        let start = labeled_button("Start practice")?;
        let handler = RoutedEventHandler::new(|_sender, _args| {
            crate::settings_process::request_practice();
            Ok(())
        });
        start
            .SetIsEnabled(std::env::args().any(|arg| arg == "--runtime"))
            .map_err(|error| error.to_string())?;
        start.Click(&handler).map_err(|error| error.to_string())?;

        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block("Quick start", 14.0)?)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block(
                "Hold the activation key to open the grid, then choose an \
                 outer then inner label. Release the leader or press Esc to close it. Tap Left Shift to open the \
                 grid without holding. Tap Left Ctrl for free \
                 mode: H/J/K/L or arrow keys move, F left-click, D right-click. While Ctrl is held, its chords \
                 go to the app untouched.",
                12.0,
            )?)
            .map_err(|error| error.to_string())?;
        let path = crate::settings_process::config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| error.to_string());
        card.Children().map_err(|error| error.to_string())?
            .Append(&text_block(&format!("Configuration: {path}\nSettings runs separately from pointer control. Invalid values are rejected before Apply."), 13.0)?).map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&start)
            .map_err(|error| error.to_string())?;
        Ok(card)
    }

    fn built_in_shortcuts_card() -> Result<StackPanel, String> {
        let card = StackPanel::new().map_err(|error| error.to_string())?;
        card.SetOrientation(Orientation::Vertical)
            .map_err(|error| error.to_string())?;
        card.SetSpacing(6.0).map_err(|error| error.to_string())?;
        let shift = text_block(
            "Left Shift: tap to open grid; held chords reach your app",
            14.0,
        )?;
        let control = text_block(
            "Left Ctrl: tap for free movement; held chords reach your app",
            14.0,
        )?;
        for child in [
            text_block("Built-in shortcuts", 14.0)?,
            text_block(
                "These shortcuts stay available unless assigned as your activation key.",
                12.0,
            )?,
        ] {
            card.Children()
                .map_err(|error| error.to_string())?
                .Append(&child)
                .map_err(|error| error.to_string())?;
        }
        for button in [&shift, &control] {
            card.Children()
                .map_err(|error| error.to_string())?
                .Append(button)
                .map_err(|error| error.to_string())?;
        }
        Ok(card)
    }

    /// One control's live value, by model key. Toggles read the check state,
    /// the choice list reads its selected index, everything else reads text.
    fn number_text(number: &NumberBox) -> Option<String> {
        use winui3::Microsoft::UI::Xaml::{DependencyObject, Media::VisualTreeHelper};
        fn input_text(node: &DependencyObject) -> Option<String> {
            if let Ok(input) = node.cast::<TextBox>() {
                return input.Text().ok().map(|text| text.to_string());
            }
            for index in 0..VisualTreeHelper::GetChildrenCount(node).ok()? {
                let child = VisualTreeHelper::GetChild(node, index).ok()?;
                if let Some(text) = input_text(&child) {
                    return Some(text);
                }
            }
            None
        }
        input_text(&number.cast().ok()?).or_else(|| number.Text().ok().map(|text| text.to_string()))
    }

    fn read_control(key: &str, field: &FieldControl) -> Option<DraftValue> {
        let control = match field {
            FieldControl::Bindings(editor) => return editor.value().ok().map(DraftValue::Text),
            FieldControl::Subgrid(rows, columns) => {
                return Some(DraftValue::Text(format!(
                    "{} x {}",
                    number_text(rows)?,
                    number_text(columns)?
                )));
            }
            FieldControl::Widget(control) => control,
        };
        if let Ok(number) = control.cast::<NumberBox>() {
            return Some(DraftValue::Text(number_text(&number)?));
        }
        if let Ok(picker) = control.cast::<ColorPicker>() {
            let color = picker.Color().ok()?;
            return Some(DraftValue::Text(format!(
                "{:02X}{:02X}{:02X}",
                color.R, color.G, color.B
            )));
        }
        match key {
            "enabled" | "nudge_enabled" | "drag_after_select" | "auto_free_mode" => {
                let toggle = Interface::cast::<ToggleSwitch>(control).ok()?;
                Some(DraftValue::Flag(toggle.IsOn().ok()?))
            }
            "leader" => {
                let combo = Interface::cast::<ComboBox>(control).ok()?;
                if let Some(key) = crate::practice::ACTIVATION_KEYS
                    .get(usize::try_from(combo.SelectedIndex().ok()?).ok()?)
                {
                    return Some(DraftValue::Text(
                        clickless_config::logical_key_name(*key).into(),
                    ));
                }
                let item = Interface::cast::<ComboBoxItem>(&combo.SelectedItem().ok()?).ok()?;
                let text = Interface::cast::<TextBlock>(&item.Content().ok()?).ok()?;
                Some(DraftValue::Text(text.Text().ok()?.to_string()))
            }
            "layout" => {
                let combo = Interface::cast::<ComboBox>(control).ok()?;
                let index = usize::try_from(combo.SelectedIndex().ok()?).ok()?;
                Some(DraftValue::Choice(index))
            }
            _ => {
                let text_box = Interface::cast::<TextBox>(control).ok()?;
                Some(DraftValue::Text(text_box.Text().ok()?.to_string()))
            }
        }
    }

    /// Control for one card, matching the kind the shared model declares. These
    /// are the only widgets `read_control` knows how to read back.
    fn make_control(descriptor: &SettingDescriptor, fields: &Fields) -> Result<Control, String> {
        let value = draft_value(fields, descriptor.key);
        if descriptor.key == "leader" {
            let combo = Interface::cast::<ComboBox>(&winui3::Microsoft::UI::Xaml::Markup::XamlReader::Load(&HSTRING::from(r#"<ComboBox xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" AutomationProperties.Name="Activation key" />"#)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let mut options = vec![
                "capslock".to_string(),
                "space".to_string(),
                "controlleft".to_string(),
                "shiftleft".to_string(),
            ];
            if !options.contains(&value) {
                options.push(value.clone());
            }
            for option in &options {
                let item = ComboBoxItem::new().map_err(|e| e.to_string())?;
                let label = crate::practice::ACTIVATION_KEYS
                    .iter()
                    .find(|key| clickless_config::logical_key_name(**key) == option)
                    .map(|key| clickless_core::home::key_name(*key).to_string())
                    .unwrap_or_else(|| option.clone());
                item.SetContent(&text_block(&label, 14.0)?)
                    .map_err(|e| e.to_string())?;
                combo
                    .Items()
                    .map_err(|e| e.to_string())?
                    .Append(&item)
                    .map_err(|e| e.to_string())?;
            }
            combo
                .SetSelectedIndex(
                    options
                        .iter()
                        .position(|option| *option == value)
                        .unwrap_or(0) as i32,
                )
                .map_err(|e| e.to_string())?;
            combo.SetWidth(240.0).map_err(|e| e.to_string())?;
            return Interface::cast::<Control>(&combo).map_err(|e| e.to_string());
        }
        match descriptor.kind {
            SettingKind::Toggle => {
                let toggle: ToggleSwitch = named_control("ToggleSwitch", descriptor.title)?;
                toggle.SetIsOn(value == "On").map_err(|e| e.to_string())?;
                toggle.cast::<Control>().map_err(|e| e.to_string())
            }
            SettingKind::Text | SettingKind::Shortcut => {
                let text_box: TextBox = named_control("TextBox", descriptor.title)?;
                text_box
                    .SetAcceptsReturn(descriptor.key == "mouse_bindings")
                    .map_err(|error| error.to_string())?;
                text_box
                    .SetText(&HSTRING::from(&value))
                    .map_err(|error| error.to_string())?;
                text_box
                    .SetWidth(240.0)
                    .map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&text_box).map_err(|error| error.to_string())
            }
            SettingKind::Number => {
                let number: NumberBox = named_control("NumberBox", descriptor.title)?;
                number
                    .SetValidationMode(NumberBoxValidationMode::Disabled)
                    .map_err(|e| e.to_string())?;
                number.SetSpinButtonPlacementMode(winui3::Microsoft::UI::Xaml::Controls::NumberBoxSpinButtonPlacementMode::Compact).map_err(|e| e.to_string())?;
                number
                    .SetText(&HSTRING::from(&value))
                    .map_err(|e| e.to_string())?;
                number.SetWidth(180.0).map_err(|e| e.to_string())?;
                number.cast::<Control>().map_err(|e| e.to_string())
            }
            SettingKind::Choice => {
                let combo = Interface::cast::<ComboBox>(&winui3::Microsoft::UI::Xaml::Markup::XamlReader::Load(&HSTRING::from(r#"<ComboBox xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" AutomationProperties.Name="Grid style" />"#)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                for (label, _draft) in CHOICE_OPTIONS {
                    let item = ComboBoxItem::new().map_err(|error| error.to_string())?;
                    item.SetContent(&text_block(label, 14.0)?)
                        .map_err(|error| error.to_string())?;
                    combo
                        .Items()
                        .map_err(|error| error.to_string())?
                        .Append(&item)
                        .map_err(|error| error.to_string())?;
                }
                let selected = CHOICE_OPTIONS
                    .iter()
                    .position(|(_label, draft)| *draft == value)
                    .unwrap_or(0);
                combo
                    .SetSelectedIndex(selected as i32)
                    .map_err(|error| error.to_string())?;
                combo.SetWidth(180.0).map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&combo).map_err(|error| error.to_string())
            }
            SettingKind::Color => {
                let picker: ColorPicker = named_control("ColorPicker", descriptor.title)?;
                picker.SetIsAlphaEnabled(false).map_err(|e| e.to_string())?;
                let (r, g, b) = clickless_config::parse_hex_rgb(descriptor.title, &value)
                    .map_err(|e| e.to_string())?;
                picker
                    .SetColor(windows::UI::Color {
                        A: 255,
                        R: r,
                        G: g,
                        B: b,
                    })
                    .map_err(|e| e.to_string())?;
                picker.cast::<Control>().map_err(|e| e.to_string())
            }
        }
    }

    fn set_control(field: &FieldControl, value: &DraftValue) -> Result<(), String> {
        let control = match field {
            FieldControl::Bindings(editor) => {
                return if let DraftValue::Text(value) = value {
                    editor.set(value)
                } else {
                    Err("Shortcut draft is invalid".into())
                };
            }
            FieldControl::Subgrid(rows, cols) => {
                if let DraftValue::Text(value) = value {
                    let (r, c) = super::split_subgrid(value)?;
                    rows.SetText(&HSTRING::from(r)).map_err(|e| e.to_string())?;
                    cols.SetText(&HSTRING::from(c)).map_err(|e| e.to_string())?;
                }
                return Ok(());
            }
            FieldControl::Widget(control) => control,
        };
        if let DraftValue::Text(value) = value {
            if let Ok(number) = control.cast::<NumberBox>() {
                return number
                    .SetText(&HSTRING::from(value))
                    .map_err(|e| e.to_string());
            }
            if let Ok(picker) = control.cast::<ColorPicker>() {
                let (r, g, b) =
                    clickless_config::parse_hex_rgb("Color", value).map_err(|e| e.to_string())?;
                return picker
                    .SetColor(windows::UI::Color {
                        A: 255,
                        R: r,
                        G: g,
                        B: b,
                    })
                    .map_err(|e| e.to_string());
            }
        }
        match value {
            DraftValue::Text(text) => {
                if let Ok(combo) = Interface::cast::<ComboBox>(control) {
                    let items = combo.Items().map_err(|e| e.to_string())?;
                    for index in 0..items.Size().map_err(|e| e.to_string())? {
                        let item = Interface::cast::<ComboBoxItem>(
                            &items.GetAt(index).map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?;
                        let label = Interface::cast::<TextBlock>(
                            &item.Content().map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?;
                        let expected = crate::practice::ACTIVATION_KEYS
                            .iter()
                            .find(|key| clickless_config::logical_key_name(**key) == text)
                            .map(|key| clickless_core::home::key_name(*key).to_string())
                            .unwrap_or_else(|| text.clone());
                        if label.Text().map_err(|e| e.to_string())? == expected.as_str() {
                            combo
                                .SetSelectedIndex(index as i32)
                                .map_err(|e| e.to_string())?;
                            break;
                        }
                    }
                    Ok(())
                } else {
                    Interface::cast::<TextBox>(control)
                        .map_err(|e| e.to_string())?
                        .SetText(&HSTRING::from(text))
                        .map_err(|e| e.to_string())
                }
            }
            DraftValue::Choice(index) => Interface::cast::<ComboBox>(control)
                .map_err(|e| e.to_string())?
                .SetSelectedIndex(*index as i32)
                .map_err(|e| e.to_string()),
            DraftValue::Flag(value) => control
                .cast::<ToggleSwitch>()
                .map_err(|e| e.to_string())?
                .SetIsOn(*value)
                .map_err(|e| e.to_string()),
        }
    }

    /// One card: title, description, live control and the model's example.
    /// Returns the control so the host can read edits back.
    fn make_card(
        descriptor: &SettingDescriptor,
        fields: &Fields,
    ) -> Result<(Grid, FieldControl, TextBlock), String> {
        let card: Grid = xaml(
            r#"<Grid Background="{ThemeResource CardBackgroundFillColorDefaultBrush}" BorderBrush="{ThemeResource CardStrokeColorDefaultBrush}" BorderThickness="1" CornerRadius="8" Padding="20,16" RowSpacing="0"><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions></Grid>"#,
        )?;
        let wide = [
            "mouse_bindings",
            "nested_keys",
            "column_keys",
            "row_keys",
            "about",
        ]
        .contains(&descriptor.key)
            || descriptor.kind == SettingKind::Color;
        let row: Grid = xaml(if wide {
            r#"<Grid RowSpacing="14"><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions></Grid>"#
        } else {
            r#"<Grid ColumnSpacing="24" RowSpacing="0"><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions></Grid>"#
        })?;
        let copy: StackPanel = xaml(r#"<StackPanel Spacing="4" VerticalAlignment="Center" />"#)?;
        let title = text_block(descriptor.title, 14.0)?;
        title
            .SetFontWeight(FontWeights::SemiBold().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        copy.Children()
            .map_err(|e| e.to_string())?
            .Append(&title)
            .map_err(|e| e.to_string())?;
        let description: TextBlock = xaml(
            r#"<TextBlock FontSize="12" TextWrapping="Wrap" Foreground="{ThemeResource TextFillColorSecondaryBrush}" />"#,
        )?;
        description
            .SetText(&HSTRING::from(if descriptor.key == "mouse_bindings" {
                "Record a key, choose its action, and remove any shortcuts you do not need."
            } else {
                descriptor.description
            }))
            .map_err(|e| e.to_string())?;
        copy.Children()
            .map_err(|e| e.to_string())?
            .Append(&description)
            .map_err(|e| e.to_string())?;
        row.Children()
            .map_err(|e| e.to_string())?
            .Append(&copy)
            .map_err(|e| e.to_string())?;
        let (view, control): (winui3::Microsoft::UI::Xaml::FrameworkElement, FieldControl) =
            if descriptor.key == "mouse_bindings" {
                let editor = crate::winui_bindings::BindingEditor::new(&draft_value(
                    fields,
                    descriptor.key,
                ))?;
                (
                    editor.view.cast().map_err(|e| e.to_string())?,
                    FieldControl::Bindings(editor),
                )
            } else if descriptor.key == "nested_size" {
                let group: StackPanel =
                    xaml(r#"<StackPanel Orientation="Horizontal" Spacing="8" />"#)?;
                let rows: NumberBox = named_control("NumberBox", "Subgrid rows")?;
                let cols: NumberBox = named_control("NumberBox", "Subgrid columns")?;
                for (number, value) in [(&rows, &fields.grid_rows), (&cols, &fields.grid_cols)] {
                    number.SetWidth(90.0).map_err(|e| e.to_string())?;
                    number
                        .SetValidationMode(NumberBoxValidationMode::Disabled)
                        .map_err(|e| e.to_string())?;
                    number
                        .SetText(&HSTRING::from(value))
                        .map_err(|e| e.to_string())?;
                }
                group
                    .Children()
                    .map_err(|e| e.to_string())?
                    .Append(&rows)
                    .map_err(|e| e.to_string())?;
                group
                    .Children()
                    .map_err(|e| e.to_string())?
                    .Append(&text_block("×", 14.0)?)
                    .map_err(|e| e.to_string())?;
                group
                    .Children()
                    .map_err(|e| e.to_string())?
                    .Append(&cols)
                    .map_err(|e| e.to_string())?;
                (
                    group.cast().map_err(|e| e.to_string())?,
                    FieldControl::Subgrid(rows, cols),
                )
            } else {
                let control = make_control(descriptor, fields)?;
                if let Ok(textbox) = control.cast::<TextBox>() {
                    textbox
                        .SetPlaceholderText(&HSTRING::from(descriptor.example))
                        .map_err(|e| e.to_string())?;
                    if wide {
                        textbox.SetWidth(f64::NAN).map_err(|e| e.to_string())?;
                        textbox
                            .SetTextWrapping(TextWrapping::Wrap)
                            .map_err(|e| e.to_string())?;
                    }
                    if descriptor.key == "about" {
                        textbox.SetIsReadOnly(true).map_err(|e| e.to_string())?;
                        textbox.SetAcceptsReturn(true).map_err(|e| e.to_string())?;
                        let path = crate::settings_process::config_path()?
                            .display()
                            .to_string();
                        let log = crate::gui_error::log_file_path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        textbox
                            .SetText(&HSTRING::from(crate::gui_error::diagnostics_text(
                                env!("CARGO_PKG_VERSION"),
                                "Windows",
                                "",
                                &format!("{path}\nLog file: {log}"),
                            )))
                            .map_err(|e| e.to_string())?;
                    }
                }
                if descriptor.kind == SettingKind::Color {
                    let expander: Expander = xaml(r#"<Expander HorizontalAlignment="Stretch" />"#)?;
                    expander
                        .SetHeader(&text_block(
                            &format!("Choose color · #{}", draft_value(fields, descriptor.key)),
                            14.0,
                        )?)
                        .map_err(|e| e.to_string())?;
                    expander.SetContent(&control).map_err(|e| e.to_string())?;
                    let target = expander.clone();
                    control
                        .cast::<ColorPicker>()
                        .map_err(|e| e.to_string())?
                        .ColorChanged(&TypedEventHandler::<
                            ColorPicker,
                            winui3::Microsoft::UI::Xaml::Controls::ColorChangedEventArgs,
                        >::new(move |sender, _| {
                            if let Some(sender) = sender.as_ref() {
                                let color = sender.Color()?;
                                target.SetHeader(
                                    &text_block(
                                        &format!(
                                            "Choose color · #{:02X}{:02X}{:02X}",
                                            color.R, color.G, color.B
                                        ),
                                        14.0,
                                    )
                                    .map_err(|_| windows_core::Error::empty())?,
                                )?;
                            }
                            Ok(())
                        }))
                        .map_err(|e| e.to_string())?;
                    (
                        expander.cast().map_err(|e| e.to_string())?,
                        FieldControl::Widget(control),
                    )
                } else {
                    (
                        control.cast().map_err(|e| e.to_string())?,
                        FieldControl::Widget(control),
                    )
                }
            };
        if wide {
            Grid::SetRow(&view, 1)
        } else {
            Grid::SetColumn(&view, 1)
        }
        .map_err(|e| e.to_string())?;
        row.Children()
            .map_err(|e| e.to_string())?
            .Append(&view)
            .map_err(|e| e.to_string())?;
        if !wide {
            let view = view.clone();
            let copy = copy.clone();
            row.SizeChanged(&winui3::Microsoft::UI::Xaml::SizeChangedEventHandler::new(
                move |sender, _| {
                    if let Some(sender) = sender.as_ref() {
                        let grid = sender.cast::<Grid>()?;
                        let narrow = grid.ActualWidth()? < 520.0;
                        grid.SetRowSpacing(if narrow { 14.0 } else { 0.0 })?;
                        Grid::SetColumn(&view, if narrow { 0 } else { 1 })?;
                        Grid::SetRow(&view, if narrow { 1 } else { 0 })?;
                        Grid::SetColumnSpan(&view, if narrow { 2 } else { 1 })?;
                        Grid::SetColumnSpan(&copy, if narrow { 2 } else { 1 })?;
                    }
                    Ok(())
                },
            ))
            .map_err(|e| e.to_string())?;
        }

        card.Children()
            .map_err(|e| e.to_string())?
            .Append(&row)
            .map_err(|e| e.to_string())?;
        let error: TextBlock = xaml(
            r#"<TextBlock FontSize="12" TextWrapping="Wrap" Foreground="{ThemeResource SystemFillColorCriticalBrush}" AutomationProperties.LiveSetting="Polite" Visibility="Collapsed" Margin="0,8,0,0" />"#,
        )?;
        Grid::SetRow(&error, 1).map_err(|e| e.to_string())?;
        card.Children()
            .map_err(|e| e.to_string())?
            .Append(&error)
            .map_err(|e| e.to_string())?;
        Ok((card, control, error))
    }
}

#[cfg(not(feature = "winui3"))]
pub mod disabled {
    pub const REASON: &str = "enable the clickless-windows/winui3 feature to use WinUI 3";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_editor::SettingsEditor;
    use clickless_config::Config;
    use clickless_config::settings_model::{SETTINGS, SettingsPage};

    #[test]
    fn card_rows_render_general_page_from_draft() {
        let editor = SettingsEditor::new(Config::default());
        let rows = card_rows(&editor, SettingsPage::General);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].key, "enabled");
        assert_eq!(rows[0].value, "On");
        assert_eq!(rows[1].key, "leader");
        assert_eq!(rows[1].value, "capslock");
        assert_eq!(rows[2].key, "hold_ms");
        assert_eq!(rows[2].value, "200");
    }

    #[test]
    fn card_rows_cover_every_modeled_key_with_a_value() {
        let editor = SettingsEditor::new(Config::default());
        for page in SettingsPage::ALL {
            let rows = card_rows(&editor, page);
            for row in &rows {
                assert!(
                    !row.value.is_empty(),
                    "page {} key {} has no draft value",
                    page.title(),
                    row.key
                );
            }
        }
        let covered: usize = SettingsPage::ALL
            .iter()
            .map(|page| card_rows(&editor, *page).len())
            .sum();
        assert_eq!(covered, SETTINGS.len());
    }

    #[test]
    fn every_page_has_a_title() {
        for page in SettingsPage::ALL {
            assert!(!page.title().is_empty());
        }
    }

    #[test]
    fn page_for_index_matches_the_sidebar_order() {
        for (index, page) in SettingsPage::ALL.iter().enumerate() {
            assert_eq!(page_for_index(index as i32), *page);
        }
        assert_eq!(page_for_index(-1), SettingsPage::General);
        assert_eq!(page_for_index(99), SettingsPage::General);
    }

    /// Every card the host draws must be readable back, except the two keys
    /// this slice has no editor for: the binding table and the About blurb.
    #[test]
    fn every_editable_card_has_a_write_back_rule() {
        let base = fields_from_config(&Config::default());
        let mut values = BTreeMap::new();
        values.insert("enabled", DraftValue::Flag(false));
        values.insert("leader", DraftValue::Text("f8".into()));
        values.insert("hold_ms", DraftValue::Text("250".into()));
        values.insert("start_speed", DraftValue::Text("400".into()));
        values.insert("max_speed", DraftValue::Text("2000".into()));
        values.insert("ramp_ms", DraftValue::Text("300".into()));
        values.insert("layout", DraftValue::Choice(1));
        values.insert("nested_size", DraftValue::Text("2 x 4".into()));
        values.insert("nudge_enabled", DraftValue::Flag(true));
        values.insert("color_panel", DraftValue::Text("101010".into()));
        values.insert("color_label", DraftValue::Text("FFFFFF".into()));
        for key in [
            "scroll_step",
            "nested_keys",
            "column_keys",
            "row_keys",
            "nudge_step",
            "panel_opacity",
            "color_border",
            "border_px",
            "color_highlight",
            "highlight_opacity",
            "color_pointer",
            "label_size",
        ] {
            values.insert(key, DraftValue::Text(draft_value(&base, key)));
        }
        values.insert(
            "drag_after_select",
            DraftValue::Flag(base.drag_after_select),
        );
        values.insert("auto_free_mode", DraftValue::Flag(base.auto_free_mode));

        for page in SettingsPage::ALL {
            for row in card_rows(&SettingsEditor::new(Config::default()), page) {
                if row.key == "about" || row.key == "mouse_bindings" {
                    continue;
                }
                assert!(
                    values.contains_key(row.key),
                    "card {} has no write-back rule",
                    row.key
                );
            }
        }

        let fields = fields_with_draft(&base, &values).expect("fields");
        assert!(!fields.enabled);
        assert!(fields.nudge_enabled);
        assert_eq!(fields.leader, "f8");
        assert_eq!(fields.hold_ms, "250");
        assert_eq!(fields.start_speed_px_s, "400");
        assert_eq!(fields.max_speed_px_s, "2000");
        assert_eq!(fields.ramp_ms, "300");
        assert_eq!(fields.layout, "simple");
        assert_eq!(fields.grid_rows, "2");
        assert_eq!(fields.grid_cols, "4");
        assert_eq!(fields.panel, "101010");
        assert_eq!(fields.label, "FFFFFF");
    }

    /// A widget the host never renders keeps the saved value, and a malformed
    /// value names its field instead of half-applying.
    #[test]
    fn fields_with_draft_keeps_untouched_fields_and_rejects_bad_values() {
        let base = fields_from_config(&Config::default());
        let mut untouched = BTreeMap::new();
        untouched.insert("leader", DraftValue::Text("f8".into()));
        let fields = fields_with_draft(&base, &untouched).expect("fields");
        assert_eq!(fields.scroll_step, base.scroll_step);
        assert_eq!(fields.grid_keys, base.grid_keys);

        let mut malformed = BTreeMap::new();
        malformed.insert("nested_size", DraftValue::Text("dense".into()));
        let error = fields_with_draft(&base, &malformed).expect_err("malformed subgrid");
        assert!(error.contains("Subgrid size"), "{error}");

        let mut unknown_choice = BTreeMap::new();
        unknown_choice.insert("layout", DraftValue::Choice(7));
        assert!(fields_with_draft(&base, &unknown_choice).is_err());
    }
}
