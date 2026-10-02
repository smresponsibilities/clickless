//! Home and safe Practice on the same WinUI dispatcher as Settings.
use crate::practice::{PRACTICE_VERSION, Practice, Step};
use crate::winui_host::enabled::{ensure_xaml_thread, text_block, window_handle, xaml};
use clickless_backend_api::OverlayBackend;
use clickless_config::Config;
pub use clickless_core::home::HomeAction;
use clickless_core::home::HomeView;
use clickless_core::{KeyEvent, LogicalKey, Phase};
use naui_winui3::Microsoft::UI::Dispatching::{DispatcherQueueHandler, DispatcherQueueTimer};
use naui_winui3::Microsoft::UI::Windowing::{AppWindow, AppWindowClosingEventArgs};
use naui_winui3::Microsoft::UI::Xaml::Controls::{Button, ComboBox, Grid, TextBlock};
use naui_winui3::Microsoft::UI::Xaml::Input::KeyEventHandler;
use naui_winui3::Microsoft::UI::Xaml::{RoutedEventHandler, Window};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows_core::{HSTRING, Interface};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, MSG, SW_HIDE, SetForegroundWindow, ShowWindow,
};

pub(crate) static PRACTICE_OPEN_REQUEST: AtomicBool = AtomicBool::new(false);
static COMPLETED_LEADER: Mutex<Option<LogicalKey>> = Mutex::new(None);

pub(crate) fn on_ui<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let dispatcher = ensure_xaml_thread()?;
    if dispatcher.HasThreadAccess().map_err(|e| e.to_string())? {
        return f();
    }
    let (tx, rx) = mpsc::channel();
    let work = Mutex::new(Some(f));
    let handler = DispatcherQueueHandler::new(move || {
        if let Some(f) = work
            .lock()
            .map_err(|_| windows_core::Error::empty())?
            .take()
        {
            let _ = tx.send(f());
        }
        Ok(())
    });
    if !dispatcher.TryEnqueue(&handler).map_err(|e| e.to_string())? {
        return Err("WinUI dispatcher is shutting down".into());
    }
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|e| format!("WinUI request failed: {e}"))?
}

struct UiWindow {
    window: Window,
    root: Grid,
    hwnd: usize,
    previous: Arc<AtomicUsize>,
    closing: i64,
}

impl UiWindow {
    fn build(title: &str, width: i32, height: i32, markup: &str) -> Result<Self, String> {
        let window = Window::new().map_err(|e| e.to_string())?;
        let root: Grid = xaml(markup)?;
        let hwnd = window_handle(&window)?;
        window
            .SetTitle(&HSTRING::from(title))
            .map_err(|e| e.to_string())?;
        set_window_content(&window, &root, title)?;
        crate::winui_host::enabled::resize(hwnd, width, height)?;
        let previous = Arc::new(AtomicUsize::new(0));
        let restore = previous.clone();
        let closing = window
            .AppWindow()
            .map_err(|e| e.to_string())?
            .Closing(
                &TypedEventHandler::<AppWindow, AppWindowClosingEventArgs>::new(
                    move |sender, args| {
                        if let Some(args) = args.as_ref() {
                            args.SetCancel(true)?;
                        }
                        if let Some(sender) = sender.as_ref() {
                            sender.Hide()?;
                        }
                        let old = restore.load(Ordering::Relaxed);
                        if old != 0 {
                            unsafe {
                                SetForegroundWindow(old as HWND);
                            }
                        }
                        Ok(())
                    },
                ),
            )
            .map_err(|e| e.to_string())?;
        crate::window_style::dark_caption(hwnd);
        attach_theme(&root)?;
        Ok(Self {
            window,
            root,
            hwnd: hwnd as usize,
            previous,
            closing,
        })
    }

    fn show(&self, focus: Button) {
        self.previous
            .store(unsafe { GetForegroundWindow() } as usize, Ordering::Relaxed);
        let window = self.window.clone();
        let root = self.root.clone();
        report(on_ui(move || {
            let theme = theme_path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .unwrap_or_else(|| "System".into());
            let requested_theme = match theme.trim() {
                "Dark" => naui_winui3::Microsoft::UI::Xaml::ElementTheme::Dark,
                "Light" => naui_winui3::Microsoft::UI::Xaml::ElementTheme::Light,
                _ => naui_winui3::Microsoft::UI::Xaml::ElementTheme::Default,
            };
            window
                .Content()
                .and_then(|content| content.cast::<Grid>())
                .and_then(|shell| shell.SetRequestedTheme(requested_theme))
                .map_err(|e| e.to_string())?;
            root.SetRequestedTheme(requested_theme)
                .map_err(|e| e.to_string())?;
            if focus.IsLoaded().map_err(|e| e.to_string())? {
                window.Activate().map_err(|e| e.to_string())?;
                focus_element(&focus)?;
            } else {
                let token = Arc::new(Mutex::new(None));
                let loaded_token = token.clone();
                let handler = RoutedEventHandler::new(move |sender, _| {
                    if let Some(sender) = sender.as_ref() {
                        let button = sender.cast::<Button>()?;
                        if let Some(token) = loaded_token
                            .lock()
                            .map_err(|_| windows_core::Error::empty())?
                            .take()
                        {
                            button.RemoveLoaded(token)?;
                        }
                        let dispatcher = button.DispatcherQueue()?;
                        dispatcher.TryEnqueue(&DispatcherQueueHandler::new(move || {
                            focus_element(&button).map_err(|_| windows_core::Error::empty())
                        }))?;
                    }
                    Ok(())
                });
                *token.lock().map_err(|_| "Focus state unavailable")? =
                    Some(focus.Loaded(&handler).map_err(|e| e.to_string())?);
                window.Activate().map_err(|e| e.to_string())?;
            }
            Ok(())
        }));
    }

    fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd as HWND, SW_HIDE);
            let previous = self.previous.load(Ordering::Relaxed);
            if previous != 0 {
                SetForegroundWindow(previous as HWND);
            }
        }
    }

    fn has_focus(&self) -> bool {
        unsafe { GetForegroundWindow() as usize == self.hwnd }
    }
}

impl Drop for UiWindow {
    fn drop(&mut self) {
        let window = self.window.clone();
        let token = self.closing;
        report(on_ui(move || {
            window
                .AppWindow()
                .map_err(|e| e.to_string())?
                .RemoveClosing(token)
                .map_err(|e| e.to_string())?;
            window.Close().map_err(|e| e.to_string())
        }));
    }
}

fn report(result: Result<(), String>) {
    if let Err(error) = result {
        crate::gui_error::log_event(&format!("WinUI: {error}"));
    }
}

fn named<T: Interface>(root: &Grid, name: &str) -> Result<T, String> {
    root.FindName(&HSTRING::from(name))
        .and_then(|value| value.cast())
        .map_err(|e| e.to_string())
}

/// Native draggable caption strip; interactive content stays outside drag region.
pub(crate) fn set_window_content(window: &Window, root: &Grid, title: &str) -> Result<(), String> {
    let shell: Grid = xaml(
        r#"<Grid Background="{ThemeResource ApplicationPageBackgroundThemeBrush}"><Grid.RowDefinitions><RowDefinition Height="32"/><RowDefinition Height="*"/></Grid.RowDefinitions></Grid>"#,
    )?;
    let caption: Grid = xaml(
        r#"<Grid xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Background="Transparent" Margin="0,0,140,0" Padding="16,0"><TextBlock x:Name="Caption" FontSize="12" VerticalAlignment="Center"/></Grid>"#,
    )?;
    caption
        .FindName(&HSTRING::from("Caption"))
        .map_err(|e| e.to_string())?
        .cast::<TextBlock>()
        .map_err(|e| e.to_string())?
        .SetText(&HSTRING::from(title))
        .map_err(|e| e.to_string())?;
    Grid::SetRow(root, 1).map_err(|e| e.to_string())?;
    let children = shell.Children().map_err(|e| e.to_string())?;
    children.Append(&caption).map_err(|e| e.to_string())?;
    children.Append(root).map_err(|e| e.to_string())?;
    attach_theme(&shell)?;
    window.SetContent(&shell).map_err(|e| e.to_string())?;
    window
        .SetExtendsContentIntoTitleBar(true)
        .map_err(|e| e.to_string())?;
    window.SetTitleBar(&caption).map_err(|e| e.to_string())
}

/// Projection for the native Focus method omitted by the bindings' FocusState enum.
/// IUIElement's fixed WinRT ABI ends with Focus, StartAnimation, StopAnimation.
pub(crate) fn focus_element<T: Interface>(control: &T) -> Result<(), String> {
    use naui_winui3::Microsoft::UI::Xaml::{IUIElement, IUIElement_Vtbl};
    if let Ok(search) = control.cast::<naui_winui3::Microsoft::UI::Xaml::Controls::AutoSuggestBox>()
    {
        use naui_winui3::Microsoft::UI::Xaml::Controls::TextBox;
        use naui_winui3::Microsoft::UI::Xaml::{DependencyObject, Media::VisualTreeHelper};
        fn find_input(node: &DependencyObject) -> Option<TextBox> {
            if let Ok(input) = node.cast::<TextBox>() {
                return Some(input);
            }
            for index in 0..VisualTreeHelper::GetChildrenCount(node).ok()? {
                if let Some(input) = find_input(&VisualTreeHelper::GetChild(node, index).ok()?) {
                    return Some(input);
                }
            }
            None
        }
        if let Some(input) = find_input(&search.cast().map_err(|e| e.to_string())?) {
            return focus_element(&input);
        }
    }
    let element = control.cast::<IUIElement>().map_err(|e| e.to_string())?;
    type Focus =
        unsafe extern "system" fn(*mut std::ffi::c_void, i32, *mut bool) -> windows_core::HRESULT;
    unsafe {
        let slots = std::mem::size_of::<IUIElement_Vtbl>() / std::mem::size_of::<usize>();
        let address = (element.vtable() as *const IUIElement_Vtbl as *const usize)
            .add(slots - 3)
            .read();
        let focus: Focus = std::mem::transmute(address);
        let mut focused = false;
        focus(element.as_raw(), 3, &mut focused)
            .ok()
            .map_err(|e| e.to_string())?;
        if focused {
            Ok(())
        } else {
            Err("Control could not receive keyboard focus".into())
        }
    }
}

fn theme_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|dir| std::path::PathBuf::from(dir).join("clickless/ui-theme"))
}

static THEME_ROOTS: Mutex<Vec<windows_core::Weak<Grid>>> = Mutex::new(Vec::new());

pub(crate) fn attach_theme(root: &Grid) -> Result<(), String> {
    use naui_winui3::Microsoft::UI::Xaml::Controls::SelectionChangedEventHandler;
    use naui_winui3::Microsoft::UI::Xaml::ElementTheme;
    let saved = theme_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| "System".into());
    let selected = match saved.trim() {
        "Light" => 1,
        "Dark" => 2,
        _ => 0,
    };
    root.SetRequestedTheme(
        [
            ElementTheme::Default,
            ElementTheme::Light,
            ElementTheme::Dark,
        ][selected],
    )
    .map_err(|e| e.to_string())?;
    THEME_ROOTS
        .lock()
        .map_err(|_| "Theme state unavailable")?
        .push(root.downgrade().map_err(|e| e.to_string())?);
    if let Ok(choice) = named::<ComboBox>(root, "UiTheme") {
        choice
            .SetSelectedIndex(selected as i32)
            .map_err(|e| e.to_string())?;
        let root = root.clone();
        choice
            .SelectionChanged(&SelectionChangedEventHandler::new(move |sender, _| {
                if let Some(sender) = sender.as_ref() {
                    let choice = sender.cast::<ComboBox>()?;
                    let index = choice.SelectedIndex()?.clamp(0, 2) as usize;
                    root.SetRequestedTheme(
                        [
                            ElementTheme::Default,
                            ElementTheme::Light,
                            ElementTheme::Dark,
                        ][index],
                    )?;
                    if let Ok(mut roots) = THEME_ROOTS.lock() {
                        roots.retain(|weak| weak.upgrade().is_some());
                        for weak in roots.iter() {
                            if let Some(root) = weak.upgrade() {
                                root.SetRequestedTheme(
                                    [
                                        ElementTheme::Default,
                                        ElementTheme::Light,
                                        ElementTheme::Dark,
                                    ][index],
                                )?;
                            }
                        }
                    }
                    if let Some(path) = theme_path() {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(error) = std::fs::write(path, ["System", "Light", "Dark"][index])
                        {
                            crate::gui_error::log_event(&format!(
                                "Could not save UI theme: {error}"
                            ));
                        }
                    }
                }
                Ok(())
            }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

const HOME_XAML: &str = r#"<Grid xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Background="{ThemeResource ApplicationPageBackgroundThemeBrush}" Padding="28" RowSpacing="24" AutomationProperties.LandmarkType="Main">
<Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="*"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>
<Grid><TextBlock Text="Clickless" Style="{ThemeResource TitleTextBlockStyle}" AutomationProperties.HeadingLevel="Level1"/><ComboBox x:Name="UiTheme" HorizontalAlignment="Right" SelectedIndex="0" AutomationProperties.Name="App theme"><ComboBoxItem Content="System"/><ComboBoxItem Content="Light"/><ComboBoxItem Content="Dark"/></ComboBox></Grid>
<ScrollViewer Grid.Row="1" HorizontalScrollBarVisibility="Disabled"><StackPanel Spacing="20" MaxWidth="900">
<Border Background="{ThemeResource CardBackgroundFillColorDefaultBrush}" CornerRadius="8" Padding="24"><StackPanel Spacing="12"><TextBlock x:Name="Status" FontSize="20" FontWeight="SemiBold" TextWrapping="Wrap"/><TextBlock x:Name="Explanation" TextWrapping="Wrap" Foreground="{ThemeResource TextFillColorSecondaryBrush}"/></StackPanel></Border>
<TextBlock Text="Point with your keyboard" Style="{ThemeResource SubtitleTextBlockStyle}" AutomationProperties.HeadingLevel="Level2"/>
<Border Background="{ThemeResource CardBackgroundFillColorDefaultBrush}" CornerRadius="8" Padding="20"><StackPanel Spacing="12"><TextBlock x:Name="Hold" TextWrapping="Wrap"/><TextBlock Text="Choose the on-screen letters to narrow the target." TextWrapping="Wrap"/><TextBlock Text="Release the activation key to return to your app. Esc cancels." TextWrapping="Wrap"/></StackPanel></Border>
</StackPanel></ScrollViewer>
<Grid Grid.Row="2" ColumnSpacing="12" RowSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="*"/></Grid.ColumnDefinitions><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions><Button x:Name="Practice" Content="Start practice" HorizontalAlignment="Stretch" Style="{StaticResource AccentButtonStyle}"/><Button x:Name="Settings" Grid.Column="1" Content="Settings" HorizontalAlignment="Stretch"/><Button x:Name="Pause" Grid.Row="1" HorizontalAlignment="Stretch"/><Button x:Name="Quit" Grid.Row="1" Grid.Column="1" Content="Quit" HorizontalAlignment="Stretch"/></Grid>
</Grid>"#;

pub struct HomeWindow {
    ui: UiWindow,
    view: Arc<Mutex<HomeView>>,
    pending: Arc<Mutex<Option<HomeAction>>>,
    practice: Button,
    settings: Button,
    leader: Mutex<LogicalKey>,
}

impl HomeWindow {
    pub fn new(enabled: bool, leader: LogicalKey, completed: bool) -> Result<Self, String> {
        on_ui(move || {
            let ui = UiWindow::build("Clickless", 760, 620, HOME_XAML)?;
            let view = Arc::new(Mutex::new(HomeView::build(enabled, leader, completed)));
            let pending = Arc::new(Mutex::new(None));
            for (name, action) in [
                ("Practice", HomeAction::StartPractice),
                ("Settings", HomeAction::OpenSettings),
                ("Pause", HomeAction::TogglePause),
                ("Quit", HomeAction::Quit),
            ] {
                let button: Button = named(&ui.root, name)?;
                let pending = pending.clone();
                button
                    .Click(&RoutedEventHandler::new(move |_, _| {
                        *pending.lock().map_err(|_| windows_core::Error::empty())? = Some(action);
                        Ok(())
                    }))
                    .map_err(|e| e.to_string())?;
            }
            let practice = named(&ui.root, "Practice")?;
            let settings = named(&ui.root, "Settings")?;
            let result = Self {
                ui,
                view,
                pending,
                practice,
                settings,
                leader: Mutex::new(leader),
            };
            result.render(leader)?;
            Ok(result)
        })
    }
    fn render(&self, leader: LogicalKey) -> Result<(), String> {
        let view = self.view.lock().map_err(|_| "Home state unavailable")?;
        named::<TextBlock>(&self.ui.root, "Status")?
            .SetText(&HSTRING::from(&view.status))
            .map_err(|e| e.to_string())?;
        named::<TextBlock>(&self.ui.root, "Explanation")?
            .SetText(&HSTRING::from(&view.explanation))
            .map_err(|e| e.to_string())?;
        named::<TextBlock>(&self.ui.root, "Hold")?
            .SetText(&HSTRING::from(format!(
                "Hold {} to open the grid.",
                clickless_core::home::key_name(leader)
            )))
            .map_err(|e| e.to_string())?;
        named::<Button>(&self.ui.root, "Pause")?
            .SetContent(&text_block(
                HomeAction::TogglePause.label(view.enabled),
                14.0,
            )?)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn show(&self) {
        self.ui
            .show(if self.view.lock().map(|v| v.first_run).unwrap_or(false) {
                self.practice.clone()
            } else {
                self.settings.clone()
            });
    }
    pub fn hide(&self) {
        self.ui.hide();
    }
    pub fn has_focus(&self) -> bool {
        self.ui.has_focus()
    }
    pub fn is_created(&self) -> bool {
        self.ui.hwnd != 0
    }
    pub fn translate_message(&self, _: &MSG) -> bool {
        false
    }
    pub fn take_action(&self) -> Option<HomeAction> {
        self.pending.lock().ok()?.take()
    }
    pub fn refresh_config(&self, config: &Config) {
        if let Ok(mut leader) = self.leader.lock() {
            *leader = config.settings.leader;
        }
        let view = self.view.clone();
        let root = self.ui.root.clone();
        let config = config.clone();
        report(on_ui(move || {
            let rebuilt = HomeView::build(
                config.enabled,
                config.settings.leader,
                config.practice_completed_version >= PRACTICE_VERSION,
            );
            named::<TextBlock>(&root, "Status")?
                .SetText(&HSTRING::from(&rebuilt.status))
                .map_err(|e| e.to_string())?;
            named::<TextBlock>(&root, "Explanation")?
                .SetText(&HSTRING::from(&rebuilt.explanation))
                .map_err(|e| e.to_string())?;
            named::<TextBlock>(&root, "Hold")?
                .SetText(&HSTRING::from(format!(
                    "Hold {} to open the grid.",
                    clickless_core::home::key_name(config.settings.leader)
                )))
                .map_err(|e| e.to_string())?;
            named::<Button>(&root, "Pause")?
                .SetContent(&text_block(
                    HomeAction::TogglePause.label(config.enabled),
                    14.0,
                )?)
                .map_err(|e| e.to_string())?;
            *view.lock().map_err(|_| "Home state unavailable")? = rebuilt;
            Ok(())
        }));
    }
    pub fn refresh(&self, enabled: bool) {
        let root = self.ui.root.clone();
        let view = self.view.clone();
        let leader = self
            .leader
            .lock()
            .map(|key| *key)
            .unwrap_or(LogicalKey::CapsLock);
        report(on_ui(move || {
            let mut view = view.lock().map_err(|_| "Home state unavailable")?;
            *view = HomeView::build(enabled, leader, !view.first_run);
            named::<Button>(&root, "Pause")?
                .SetContent(&text_block(HomeAction::TogglePause.label(enabled), 14.0)?)
                .map_err(|e| e.to_string())?;
            named::<TextBlock>(&root, "Status")?
                .SetText(&HSTRING::from(&view.status))
                .map_err(|e| e.to_string())?;
            named::<TextBlock>(&root, "Explanation")?
                .SetText(&HSTRING::from(&view.explanation))
                .map_err(|e| e.to_string())?;
            Ok(())
        }));
    }
}

const PRACTICE_XAML: &str = r#"<Grid xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Background="{ThemeResource ApplicationPageBackgroundThemeBrush}" Padding="28" RowSpacing="24" AutomationProperties.LandmarkType="Main">
<Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="*"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>
<Grid><TextBlock Text="Practice" Style="{ThemeResource TitleTextBlockStyle}" AutomationProperties.HeadingLevel="Level1"/><ComboBox x:Name="UiTheme" HorizontalAlignment="Right" AutomationProperties.Name="App theme"><ComboBoxItem Content="System"/><ComboBoxItem Content="Light"/><ComboBoxItem Content="Dark"/></ComboBox></Grid>
<ScrollViewer Grid.Row="1"><StackPanel Spacing="20"><TextBlock Text="Learn without moving or clicking your real pointer." TextWrapping="Wrap" Foreground="{ThemeResource TextFillColorSecondaryBrush}"/><ProgressBar x:Name="Progress" Maximum="3" Value="0" AutomationProperties.Name="Lesson progress"/><Border Background="{ThemeResource CardBackgroundFillColorDefaultBrush}" CornerRadius="8" Padding="24"><StackPanel Spacing="16"><TextBlock x:Name="Instruction" FontSize="20" TextWrapping="Wrap" AutomationProperties.HeadingLevel="Level2" Text="Choose Start to learn your activation key and target selection."/><TextBlock x:Name="Status" TextWrapping="Wrap" Foreground="{ThemeResource TextFillColorSecondaryBrush}"/></StackPanel></Border></StackPanel></ScrollViewer>
<Grid Grid.Row="2" ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="*"/><ColumnDefinition Width="*"/></Grid.ColumnDefinitions><Button x:Name="Start" Content="Start" HorizontalAlignment="Stretch" Style="{StaticResource AccentButtonStyle}"/><Button x:Name="Skip" Grid.Column="1" Content="Skip" HorizontalAlignment="Stretch"/><Button x:Name="Finish" Grid.Column="2" Content="Finish" HorizontalAlignment="Stretch" IsEnabled="False"/></Grid>
</Grid>"#;

struct PracticeState {
    lesson: Option<Practice>,
    overlay: crate::overlay::WindowsOverlay,
    started: Instant,
    root: Grid,
    hwnd: usize,
}
impl PracticeState {
    fn render(&mut self) -> Result<(), String> {
        use naui_winui3::Microsoft::UI::Xaml::Controls::ProgressBar;
        let (instruction, status, progress, done) = match self.lesson.as_ref() {
            Some(p) => (
                p.instruction(),
                p.status(),
                match p.step() {
                    Step::SelectActivationKey => 0.0,
                    Step::HoldLeader => 1.0,
                    Step::GridPick => 2.0,
                    Step::Done => 3.0,
                },
                p.step() == Step::Done,
            ),
            None => (
                "Choose Start to learn your activation key and target selection.".into(),
                "Esc closes Practice.".into(),
                0.0,
                false,
            ),
        };
        named::<TextBlock>(&self.root, "Instruction")?
            .SetText(&HSTRING::from(instruction))
            .map_err(|e| e.to_string())?;
        named::<TextBlock>(&self.root, "Status")?
            .SetText(&HSTRING::from(status))
            .map_err(|e| e.to_string())?;
        named::<Button>(&self.root, "Finish")?
            .SetIsEnabled(done)
            .map_err(|e| e.to_string())?;
        named::<ProgressBar>(&self.root, "Progress")?
            .SetValue(progress)
            .map_err(|e| e.to_string())?;
        if let Some(frame) = self.lesson.as_ref().and_then(Practice::grid_overlay) {
            self.overlay.show(&frame).map_err(|e| e.to_string())?;
        } else {
            self.overlay.hide().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn cancel(&mut self) {
        if let Some(p) = self.lesson.as_mut() {
            p.cancel();
        }
        report(self.render());
    }
}

fn new_practice() -> Practice {
    match crate::settings_process::config_path()
        .ok()
        .and_then(|path| Config::load_from_file(&path).ok())
    {
        Some(config) if config.practice_completed_version > 0 => {
            Practice::with_setup(config.settings.leader, config.grid.dense)
        }
        _ => Practice::new(),
    }
}

pub struct PracticeWindow {
    ui: UiWindow,
    state: Arc<Mutex<PracticeState>>,
    start: Button,
    timer: DispatcherQueueTimer,
}
impl PracticeWindow {
    pub fn new() -> Result<Self, String> {
        on_ui(|| {
            let ui = UiWindow::build("Clickless Practice", 800, 650, PRACTICE_XAML)?;
            let state = Arc::new(Mutex::new(PracticeState {
                lesson: None,
                overlay: crate::overlay::WindowsOverlay::new()?,
                started: Instant::now(),
                root: ui.root.clone(),
                hwnd: ui.hwnd,
            }));
            let start: Button = named(&ui.root, "Start")?;
            let data = state.clone();
            let focus = start.clone();
            start
                .Click(&RoutedEventHandler::new(move |_, _| {
                    let mut state = data.lock().map_err(|_| windows_core::Error::empty())?;
                    state.lesson = Some(new_practice());
                    state.started = Instant::now();
                    report(state.render());
                    focus_element(&focus).map_err(|_| windows_core::Error::empty())?;
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
            let skip: Button = named(&ui.root, "Skip")?;
            let data = state.clone();
            let handle = ui.hwnd;
            skip.Click(&RoutedEventHandler::new(move |_, _| {
                data.lock()
                    .map_err(|_| windows_core::Error::empty())?
                    .cancel();
                unsafe {
                    ShowWindow(handle as HWND, SW_HIDE);
                }
                Ok(())
            }))
            .map_err(|e| e.to_string())?;
            let finish: Button = named(&ui.root, "Finish")?;
            let data = state.clone();
            let handle = ui.hwnd;
            finish
                .Click(&RoutedEventHandler::new(move |_, _| {
                    let state = data.lock().map_err(|_| windows_core::Error::empty())?;
                    if let Some(lesson) = state.lesson.as_ref().filter(|p| p.step() == Step::Done) {
                        let result = persist_completion(lesson.chosen_leader());
                        match result {
                            Ok(()) => unsafe {
                                ShowWindow(handle as HWND, SW_HIDE);
                            },
                            Err(error) => {
                                named::<TextBlock>(&state.root, "Status")
                                    .map_err(|_| windows_core::Error::empty())?
                                    .SetText(&HSTRING::from(error))?;
                            }
                        }
                    }
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
            for press in [true, false] {
                let data = state.clone();
                let handler = KeyEventHandler::new(move |_, args| {
                    let Some(args) = args.as_ref() else {
                        return Ok(());
                    };
                    let vk = args.Key()?.0 as u32;
                    let mut state = data.lock().map_err(|_| windows_core::Error::empty())?;
                    if vk == 0x1B {
                        state.cancel();
                        unsafe {
                            ShowWindow(state.hwnd as HWND, SW_HIDE);
                        }
                        args.SetHandled(true)?;
                        return Ok(());
                    }
                    if vk == 0x09 || vk == 0x0D {
                        return Ok(());
                    }
                    let now = state.started.elapsed().as_millis() as u64;
                    if let Some(key) = crate::scancode::vk_to_logical(vk)
                        && let Some(p) = state
                            .lesson
                            .as_mut()
                            .filter(|p| !p.cancelled() && p.step() != Step::Done)
                    {
                        p.key(
                            KeyEvent::new(key, if press { Phase::Press } else { Phase::Release }),
                            now,
                        );
                        report(state.render());
                        args.SetHandled(true)?;
                    }
                    Ok(())
                });
                if press {
                    ui.root.PreviewKeyDown(&handler)
                } else {
                    ui.root.PreviewKeyUp(&handler)
                }
                .map_err(|e| e.to_string())?;
            }
            let timer = ensure_xaml_thread()?
                .CreateTimer()
                .map_err(|e| e.to_string())?;
            timer
                .SetInterval(TimeSpan { Duration: 500_000 })
                .map_err(|e| e.to_string())?;
            let data = state.clone();
            timer
                .Tick(&TypedEventHandler::new(move |_, _| {
                    let mut state = data.lock().map_err(|_| windows_core::Error::empty())?;
                    if unsafe { GetForegroundWindow() as usize != state.hwnd }
                        && state
                            .lesson
                            .as_ref()
                            .is_some_and(|p| !p.cancelled() && p.step() != Step::Done)
                    {
                        state.cancel();
                    }
                    Ok(())
                }))
                .map_err(|e| e.to_string())?;
            timer.Start().map_err(|e| e.to_string())?;
            Ok(Self {
                ui,
                state,
                start,
                timer,
            })
        })
    }
    pub fn show(&self) {
        self.ui.show(self.start.clone());
    }
    pub fn hide(&self) {
        let data = self.state.clone();
        report(on_ui(move || {
            data.lock()
                .map_err(|_| "Practice state unavailable")?
                .cancel();
            Ok(())
        }));
        self.ui.hide();
    }
    pub fn has_focus(&self) -> bool {
        self.ui.has_focus()
    }
    pub fn is_created(&self) -> bool {
        self.ui.hwnd != 0
    }
    pub fn translate_message(&self, _: &MSG) -> bool {
        false
    }
    pub fn reset_state(&self) {
        let data = self.state.clone();
        report(on_ui(move || {
            let mut state = data.lock().map_err(|_| "Practice state unavailable")?;
            state.lesson = None;
            state.render()
        }));
    }
}
impl Drop for PracticeWindow {
    fn drop(&mut self) {
        let timer = self.timer.clone();
        let data = self.state.clone();
        report(on_ui(move || {
            timer.Stop().map_err(|e| e.to_string())?;
            data.lock()
                .map_err(|_| "Practice state unavailable")?
                .overlay
                .hide()
                .map_err(|e| e.to_string())
        }));
    }
}
fn persist_completion(leader: LogicalKey) -> Result<(), String> {
    let path = crate::settings_process::config_path()?;
    let mut config = if path.exists() {
        Config::load_from_file(&path).map_err(|e| e.to_string())?
    } else {
        Config::default()
    };
    config.settings.leader = leader;
    config.practice_completed_version = PRACTICE_VERSION;
    config.validate().map_err(|e| e.to_string())?;
    config.save_to_file(&path).map_err(|e| e.to_string())?;
    *COMPLETED_LEADER
        .lock()
        .map_err(|_| "Practice completion unavailable")? = Some(leader);
    Ok(())
}
pub(crate) fn take_completed_leader() -> Option<LogicalKey> {
    COMPLETED_LEADER.lock().ok()?.take()
}

// Existing Home and Practice checks moved from HWND children to XAML controls.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_home_window_creates_with_four_buttons() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        assert!(window.is_created());
        let root = window.ui.root.clone();
        on_ui(move || {
            for name in ["Practice", "Settings", "Pause", "Quit"] {
                named::<Button>(&root, name)?;
            }
            Ok(())
        })
        .expect("four Home actions");
    }

    #[test]
    fn t02_status_control_exists_and_view_names_the_saved_key() {
        let window = HomeWindow::new(true, LogicalKey::Space, true).expect("create home window");
        assert!(
            window
                .view
                .lock()
                .expect("Home view")
                .status
                .contains(clickless_core::home::key_name(LogicalKey::Space))
        );
    }

    #[test]
    fn t03_an_action_is_delivered_exactly_once() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        assert_eq!(window.take_action(), None);
        *window.pending.lock().expect("Home actions") = Some(HomeAction::OpenSettings);
        assert_eq!(window.take_action(), Some(HomeAction::OpenSettings));
        assert_eq!(window.take_action(), None);
    }

    #[test]
    fn t04_pause_toggle_updates_the_view_for_the_next_refresh() {
        let window = HomeWindow::new(true, LogicalKey::CapsLock, true).expect("create home window");
        window.refresh(false);
        assert!(
            window
                .view
                .lock()
                .expect("Home view")
                .button_labels()
                .contains(&"Resume pointer control")
        );
    }

    #[test]
    fn practice_window_creates_with_start_skip_finish() {
        let window = PracticeWindow::new().expect("create practice window");
        assert!(window.is_created());
        let root = window.ui.root.clone();
        on_ui(move || {
            for name in ["Start", "Skip", "Finish"] {
                named::<Button>(&root, name)?;
            }
            Ok(())
        })
        .expect("three Practice actions");
    }
}
