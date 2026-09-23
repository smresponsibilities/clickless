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
        "leader" => fields.leader.clone(),
        "hold_ms" => fields.hold_ms.clone(),
        "start_speed" => fields.start_speed_px_s.clone(),
        "max_speed" => fields.max_speed_px_s.clone(),
        "ramp_ms" => fields.ramp_ms.clone(),
        "layout" => fields.layout.clone(),
        "nested_size" => format!("{} x {}", fields.grid_rows, fields.grid_cols),
        "nudge_enabled" => on_off(fields.nudge_enabled),
        "mouse_bindings" => format!("{} bindings", fields.mouse_bindings.len()),
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
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex, Once, OnceLock};

    use windows::Foundation::{IReference, Uri};
    use windows_core::{
        Array, HSTRING, IInspectable_Vtbl, Interface, Ref, imp::WeakRefCount, implement,
    };
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOZORDER, SetForegroundWindow, SetWindowPos,
    };
    use winui3::Microsoft::UI::Dispatching::{DispatcherQueue, DispatcherQueueHandler};
    use winui3::Microsoft::UI::Text::FontWeights;
    use winui3::Microsoft::UI::Xaml::Controls::{
        Button, CheckBox, ComboBox, ComboBoxItem, Control, ListBox, ListBoxItem, Orientation,
        ScrollViewer, SelectionChangedEventHandler, StackPanel, TextBlock, TextBox,
        XamlControlsResources,
    };
    use winui3::Microsoft::UI::Xaml::Markup::{
        IXamlMetadataProvider, IXamlMetadataProvider_Impl, IXamlType, XmlnsDefinition,
    };
    use winui3::Microsoft::UI::Xaml::XamlTypeInfo::XamlControlsXamlMetaDataProvider;
    use winui3::Microsoft::UI::Xaml::{
        Application, ApplicationInitializationCallback, HorizontalAlignment, IApplicationFactory,
        IApplicationFactory_Vtbl, IApplicationOverrides, IApplicationOverrides_Impl,
        LaunchActivatedEventArgs, ResourceDictionary, RoutedEventHandler, TextWrapping, Thickness,
        UIElement, Window,
    };
    use winui3::Windows::UI::Xaml::Interop::TypeName;
    use winui3::bootstrap::{PackageDependency, WindowsAppSDKVersion};
    use winui3::{
        ApartmentType, ChildClass, ChildClassImpl, Compose, CreateInstanceFn, IWindowNative,
        init_apartment,
    };

    /// Settings window design size at 96 dpi, matching the native shell.
    const SETTINGS_WIDTH: i32 = 820;
    const SETTINGS_HEIGHT: i32 = 640;

    const LOCK_FAILED: &str = "settings state was poisoned";

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

    /// Starts the XAML thread on first use and waits for its dispatcher queue.
    /// A bare `Application::new()` plus a manually created dispatcher queue is
    /// not a valid WinUI 3 host: `Window::new` fails with RPC_E_WRONG_THREAD
    /// (0x8001010E). The working sequence is STA apartment, bootstrap
    /// dependency, then `Application::Start` with a composed `Application`
    /// subclass carrying a XAML metadata provider.
    fn ensure_xaml_thread() -> Result<DispatcherQueue, String> {
        XAML_SPAWN.call_once(|| {
            let spawned = std::thread::Builder::new()
                .name("clickless-xaml".to_string())
                .spawn(xaml_thread_main);
            if let Err(error) = spawned {
                let _ = XAML_DISPATCHER.set(Err(format!("XAML thread spawn failed: {error}")));
            }
        });
        XAML_DISPATCHER.wait().clone()
    }

    /// XAML thread body. Everything XAML happens here: apartment, bootstrap,
    /// `Application::Start` (which pumps until the process exits).
    fn xaml_thread_main() {
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
        init_apartment(ApartmentType::SingleThreaded)
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
    struct Shared {
        editor: Mutex<SettingsEditor>,
        cards: Mutex<StackPanel>,
        /// Live control per model key for the visible page.
        controls: Mutex<BTreeMap<&'static str, Control>>,
        sidebar: ListBox,
        status: TextBlock,
        apply: Mutex<ApplyCallback>,
    }

    impl Shared {
        /// Reads every live card back into the draft. A malformed value names
        /// its field and leaves the draft untouched, like the native shell.
        fn sync_draft(&self) -> Result<(), String> {
            let values = self.read_controls();
            let mut editor = self.editor.lock().map_err(|_| LOCK_FAILED)?;
            let base = fields_from_config(editor.draft());
            let fields = fields_with_draft(&base, &values)?;
            editor.edit(&fields)
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
            self.apply()?;
            let path =
                clickless_config::default_config_path().map_err(|error| error.to_string())?;
            let editor = self.editor.lock().map_err(|_| LOCK_FAILED)?;
            editor.save(&path).map_err(|error| error.to_string())?;
            Ok(path.display().to_string())
        }

        fn reset_all(&self) -> Result<(), String> {
            self.editor.lock().map_err(|_| LOCK_FAILED)?.reset_all();
            Ok(())
        }

        fn cancel(&self) -> Result<(), String> {
            self.editor.lock().map_err(|_| LOCK_FAILED)?.cancel();
            Ok(())
        }

        fn set_status(&self, text: &str) {
            let _ = self.status.SetText(&HSTRING::from(text));
        }

        /// Redraws the page the sidebar selects, from the current draft.
        fn render_current_page(&self) {
            let page = match self.sidebar.SelectedIndex() {
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
            for descriptor in settings_model::settings_for(page) {
                let (card, control) = make_card(descriptor, &fields)?;
                controls.insert(descriptor.key, control);
                cards
                    .Children()
                    .map_err(|error| error.to_string())?
                    .Append(&card)
                    .map_err(|error| error.to_string())?;
            }
            if page == SettingsPage::About {
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
                .recv()
                .map_err(|_| "XAML thread exited before building settings".to_string())??;
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
        resize(hwnd)?;
        window
            .SetTitle(&HSTRING::from("Clickless Settings"))
            .map_err(|error| error.to_string())?;

        let root = StackPanel::new().map_err(|error| error.to_string())?;
        root.SetOrientation(Orientation::Horizontal)
            .map_err(|error| error.to_string())?;

        let sidebar = ListBox::new().map_err(|error| error.to_string())?;
        sidebar.SetWidth(160.0).map_err(|error| error.to_string())?;
        sidebar
            .SetHorizontalContentAlignment(HorizontalAlignment::Left)
            .map_err(|error| error.to_string())?;
        for page in SettingsPage::ALL {
            let item = ListBoxItem::new().map_err(|error| error.to_string())?;
            item.SetContent(&text_block(page.title(), 14.0)?)
                .map_err(|error| error.to_string())?;
            sidebar
                .Items()
                .map_err(|error| error.to_string())?
                .Append(&item)
                .map_err(|error| error.to_string())?;
        }
        sidebar
            .SetSelectedIndex(0)
            .map_err(|error| error.to_string())?;
        root.Children()
            .map_err(|error| error.to_string())?
            .Append(&sidebar)
            .map_err(|error| error.to_string())?;

        let cards = StackPanel::new().map_err(|error| error.to_string())?;
        cards
            .SetOrientation(Orientation::Vertical)
            .map_err(|error| error.to_string())?;
        cards.SetSpacing(12.0).map_err(|error| error.to_string())?;
        cards
            .SetMargin(Thickness {
                Left: 16.0,
                Top: 12.0,
                Right: 16.0,
                Bottom: 12.0,
            })
            .map_err(|error| error.to_string())?;
        let scroller = ScrollViewer::new().map_err(|error| error.to_string())?;
        scroller
            .SetContent(&cards)
            .map_err(|error| error.to_string())?;
        let status = text_block("Ready.", 12.0)?;

        let shared = Arc::new(Shared {
            editor: Mutex::new(SettingsEditor::new(seed)),
            cards: Mutex::new(cards),
            controls: Mutex::new(BTreeMap::new()),
            sidebar: sidebar.clone(),
            status: status.clone(),
            apply: Mutex::new(on_apply),
        });

        let column = StackPanel::new().map_err(|error| error.to_string())?;
        column
            .SetOrientation(Orientation::Vertical)
            .map_err(|error| error.to_string())?;
        column
            .Children()
            .map_err(|error| error.to_string())?
            .Append(&scroller)
            .map_err(|error| error.to_string())?;
        column
            .Children()
            .map_err(|error| error.to_string())?
            .Append(&footer(&shared)?)
            .map_err(|error| error.to_string())?;
        column
            .Children()
            .map_err(|error| error.to_string())?
            .Append(&status)
            .map_err(|error| error.to_string())?;
        root.Children()
            .map_err(|error| error.to_string())?
            .Append(&column)
            .map_err(|error| error.to_string())?;

        {
            let shared = Arc::clone(&shared);
            let handler = SelectionChangedEventHandler::new(move |_sender, _args| {
                shared.render_current_page();
                Ok(())
            });
            sidebar
                .SelectionChanged(&handler)
                .map_err(|error| error.to_string())?;
        }

        window
            .SetContent(&root)
            .map_err(|error| error.to_string())?;
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
    fn text_block(text: &str, size: f64) -> Result<TextBlock, String> {
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

    fn labeled_button(label: &str) -> Result<Button, String> {
        let button = Button::new().map_err(|error| error.to_string())?;
        button
            .SetContent(&text_block(label, 14.0)?)
            .map_err(|error| error.to_string())?;
        Ok(button)
    }

    /// Native top-level handle, used for focus detection and placement.
    fn window_handle(window: &Window) -> Result<HWND, String> {
        let native = Interface::cast::<IWindowNative>(window).map_err(|error| error.to_string())?;
        let handle = unsafe { native.WindowHandle() }.map_err(|error| error.to_string())?;
        if handle.0.is_null() {
            return Err("WinUI window reported no native handle".to_string());
        }
        Ok(handle.0 as HWND)
    }

    /// Design size scaled by the system dpi, like the native shell.
    fn resize(hwnd: HWND) -> Result<(), String> {
        let dpi = unsafe { GetDpiForSystem() } as i32;
        let applied = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                SETTINGS_WIDTH * dpi / 96,
                SETTINGS_HEIGHT * dpi / 96,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
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
                Ok(message) => message,
                Err(error) => error,
            };
            target.set_status(&message);
            target.render_current_page();
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
        let start = labeled_button("Practice again")?;
        let handler = RoutedEventHandler::new(|_sender, _args| {
            crate::practice_dialog::PRACTICE_OPEN_REQUEST.store(true, Ordering::SeqCst);
            Ok(())
        });
        start.Click(&handler).map_err(|error| error.to_string())?;

        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block("Practice", 14.0)?)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block(
                "Three steps on the real engine: hold the activation key, move the pointer, then \
                 pick a subgrid target. Esc leaves at any point and nothing is clicked.",
                12.0,
            )?)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&start)
            .map_err(|error| error.to_string())?;
        Ok(card)
    }

    /// One control's live value, by model key. Toggles read the check state,
    /// the choice list reads its selected index, everything else reads text.
    fn read_control(key: &str, control: &Control) -> Option<DraftValue> {
        match key {
            "enabled" | "nudge_enabled" => {
                let toggle = Interface::cast::<CheckBox>(control).ok()?;
                let checked = toggle.IsChecked().ok()?;
                Some(DraftValue::Flag(checked.Value().ok()?))
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
        match descriptor.kind {
            SettingKind::Toggle => {
                let toggle = CheckBox::new().map_err(|error| error.to_string())?;
                let is_on = value.parse::<bool>().unwrap_or(false);
                let is_on_ref = Interface::cast::<IReference<bool>>(
                    &windows::Foundation::PropertyValue::CreateBoolean(is_on)
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let checkbox =
                    Interface::cast::<CheckBox>(&toggle).map_err(|error| error.to_string())?;
                checkbox
                    .SetIsChecked(&is_on_ref)
                    .map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&toggle).map_err(|error| error.to_string())
            }
            SettingKind::Text | SettingKind::Shortcut => {
                let text_box = TextBox::new().map_err(|error| error.to_string())?;
                text_box
                    .SetText(&HSTRING::from(&value))
                    .map_err(|error| error.to_string())?;
                text_box
                    .SetWidth(240.0)
                    .map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&text_box).map_err(|error| error.to_string())
            }
            SettingKind::Number => {
                let text_box = TextBox::new().map_err(|error| error.to_string())?;
                text_box
                    .SetText(&HSTRING::from(&value))
                    .map_err(|error| error.to_string())?;
                text_box
                    .SetWidth(140.0)
                    .map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&text_box).map_err(|error| error.to_string())
            }
            SettingKind::Choice => {
                let combo = ComboBox::new().map_err(|error| error.to_string())?;
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
                let text_box = TextBox::new().map_err(|error| error.to_string())?;
                text_box
                    .SetText(&HSTRING::from(&value))
                    .map_err(|error| error.to_string())?;
                text_box
                    .SetWidth(140.0)
                    .map_err(|error| error.to_string())?;
                Interface::cast::<Control>(&text_box).map_err(|error| error.to_string())
            }
        }
    }

    /// One card: title, description, live control and the model's example.
    /// Returns the control so the host can read edits back.
    fn make_card(
        descriptor: &SettingDescriptor,
        fields: &Fields,
    ) -> Result<(StackPanel, Control), String> {
        let card = StackPanel::new().map_err(|error| error.to_string())?;
        card.SetOrientation(Orientation::Vertical)
            .map_err(|error| error.to_string())?;
        card.SetSpacing(4.0).map_err(|error| error.to_string())?;
        card.SetMargin(Thickness {
            Left: 4.0,
            Top: 4.0,
            Right: 4.0,
            Bottom: 4.0,
        })
        .map_err(|error| error.to_string())?;

        let title = text_block(descriptor.title, 15.0)?;
        title
            .SetFontWeight(FontWeights::SemiBold().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&title)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block(descriptor.description, 12.0)?)
            .map_err(|error| error.to_string())?;
        let control = make_control(descriptor, fields)?;
        let element = Interface::cast::<UIElement>(&control).map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&element)
            .map_err(|error| error.to_string())?;
        card.Children()
            .map_err(|error| error.to_string())?
            .Append(&text_block(
                &format!("Example: {}", descriptor.example),
                11.0,
            )?)
            .map_err(|error| error.to_string())?;
        Ok((card, control))
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
