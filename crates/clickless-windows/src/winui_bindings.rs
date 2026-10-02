//! Typed pointer bindings with local key capture and duplicate feedback.
use crate::winui_host::enabled::{labeled_button, text_block, xaml};
use naui_winui3::Microsoft::UI::Xaml::Controls::{
    ComboBox, ComboBoxItem, Grid, StackPanel, TextBlock,
};
use naui_winui3::Microsoft::UI::Xaml::Input::KeyEventHandler;
use naui_winui3::Microsoft::UI::Xaml::{RoutedEventHandler, Visibility};
use std::sync::{Arc, Mutex};
use windows_core::{HSTRING, Interface};

const ACTIONS: &[(&str, &str)] = &[
    ("Move left", "move_left"),
    ("Move right", "move_right"),
    ("Move up", "move_up"),
    ("Move down", "move_down"),
    ("Left click", "click_left"),
    ("Right click", "click_right"),
    ("Scroll up", "scroll_up"),
    ("Scroll down", "scroll_down"),
    ("Speed up", "speed_up"),
    ("Speed down", "speed_down"),
    ("Open grid", "enter_grid"),
];

struct Row {
    key: Arc<Mutex<String>>,
    choice: ComboBox,
    removed: Arc<Mutex<bool>>,
}

#[derive(Clone)]
pub(crate) struct BindingEditor {
    pub view: StackPanel,
    rows: Arc<Mutex<Vec<Row>>>,
    list: StackPanel,
    error: TextBlock,
}

impl BindingEditor {
    pub fn new(value: &str) -> Result<Self, String> {
        let view: StackPanel = xaml(r#"<StackPanel Spacing="12" />"#)?;
        let list: StackPanel = xaml(r#"<StackPanel Spacing="8" />"#)?;
        let error: TextBlock = xaml(
            r#"<TextBlock TextWrapping="Wrap" Foreground="{ThemeResource SystemFillColorCriticalBrush}" AutomationProperties.LiveSetting="Polite" />"#,
        )?;
        let add = labeled_button("Add shortcut")?;
        for child in [
            list.cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
            error
                .cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
            add.cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
        ] {
            view.Children()
                .map_err(|e| e.to_string())?
                .Append(&child)
                .map_err(|e| e.to_string())?;
        }
        let editor = Self {
            view,
            rows: Arc::new(Mutex::new(Vec::new())),
            list,
            error,
        };
        editor.set(value)?;
        let target = editor.clone();
        add.Click(&RoutedEventHandler::new(move |_, _| {
            target
                .add("", "move_left")
                .map_err(|_| windows_core::Error::empty())?;
            Ok(())
        }))
        .map_err(|e| e.to_string())?;
        Ok(editor)
    }
    pub fn set(&self, value: &str) -> Result<(), String> {
        self.list
            .Children()
            .map_err(|e| e.to_string())?
            .Clear()
            .map_err(|e| e.to_string())?;
        self.rows
            .lock()
            .map_err(|_| "Shortcut state unavailable")?
            .clear();
        for line in value
            .split(['\r', '\n'])
            .filter(|line| !line.trim().is_empty())
        {
            let (key, action) = line
                .split_once('=')
                .ok_or("Shortcut must contain a key and action")?;
            self.add(key.trim(), action.trim())?;
        }
        Ok(())
    }
    fn add(&self, initial_key: &str, action: &str) -> Result<(), String> {
        let row: Grid = xaml(
            r#"<Grid ColumnSpacing="12"><Grid.ColumnDefinitions><ColumnDefinition Width="120"/><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions></Grid>"#,
        )?;
        let key = Arc::new(Mutex::new(initial_key.to_string()));
        let record = labeled_button(if initial_key.is_empty() {
            "Record key"
        } else {
            initial_key
        })?;
        record
            .SetHorizontalAlignment(naui_winui3::Microsoft::UI::Xaml::HorizontalAlignment::Stretch)
            .map_err(|e| e.to_string())?;
        let choice: ComboBox = xaml(
            r#"<ComboBox HorizontalAlignment="Stretch" AutomationProperties.Name="Pointer action" />"#,
        )?;
        let remove = labeled_button("Remove")?;
        Grid::SetColumn(&choice, 1).map_err(|e| e.to_string())?;
        Grid::SetColumn(&remove, 2).map_err(|e| e.to_string())?;
        for (label, _) in ACTIONS {
            let item = ComboBoxItem::new().map_err(|e| e.to_string())?;
            item.SetContent(&text_block(label, 14.0)?)
                .map_err(|e| e.to_string())?;
            choice
                .Items()
                .map_err(|e| e.to_string())?
                .Append(&item)
                .map_err(|e| e.to_string())?;
        }
        let selected = ACTIONS
            .iter()
            .position(|(_, value)| *value == action)
            .ok_or_else(|| format!("Unsupported pointer action: {action}"))?;
        choice
            .SetSelectedIndex(selected as i32)
            .map_err(|e| e.to_string())?;
        let recording = Arc::new(Mutex::new(false));
        let active = recording.clone();
        let button = record.clone();
        record
            .Click(&RoutedEventHandler::new(move |_, _| {
                *active.lock().map_err(|_| windows_core::Error::empty())? = true;
                button.SetContent(
                    &text_block("Press a key; Esc cancels", 14.0)
                        .map_err(|_| windows_core::Error::empty())?,
                )?;
                crate::winui_windows::focus_element(&button)
                    .map_err(|_| windows_core::Error::empty())?;
                Ok(())
            }))
            .map_err(|e| e.to_string())?;
        let active = recording;
        let stored = key.clone();
        let button = record.clone();
        let editor = self.clone();
        record
            .PreviewKeyDown(&KeyEventHandler::new(move |_, args| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };
                let mut active = active.lock().map_err(|_| windows_core::Error::empty())?;
                if !*active {
                    return Ok(());
                }
                let vk = args.Key()?.0 as u32;
                if vk != 0x09
                    && vk != 0x1B
                    && let Some(logical) = crate::scancode::vk_to_logical(vk)
                {
                    *stored.lock().map_err(|_| windows_core::Error::empty())? =
                        clickless_config::logical_key_name(logical).into();
                }
                *active = false;
                let label = stored
                    .lock()
                    .map_err(|_| windows_core::Error::empty())?
                    .clone();
                button.SetContent(
                    &text_block(
                        if label.is_empty() {
                            "Record key"
                        } else {
                            &label
                        },
                        14.0,
                    )
                    .map_err(|_| windows_core::Error::empty())?,
                )?;
                args.SetHandled(vk != 0x09)?;
                editor.feedback();
                Ok(())
            }))
            .map_err(|e| e.to_string())?;
        let removed = Arc::new(Mutex::new(false));
        let deleted = removed.clone();
        let container = row.clone();
        let editor = self.clone();
        remove
            .Click(&RoutedEventHandler::new(move |_, _| {
                *deleted.lock().map_err(|_| windows_core::Error::empty())? = true;
                container.SetVisibility(Visibility::Collapsed)?;
                editor.feedback();
                Ok(())
            }))
            .map_err(|e| e.to_string())?;
        for child in [
            record
                .cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
            choice
                .cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
            remove
                .cast::<naui_winui3::Microsoft::UI::Xaml::UIElement>()
                .map_err(|e| e.to_string())?,
        ] {
            row.Children()
                .map_err(|e| e.to_string())?
                .Append(&child)
                .map_err(|e| e.to_string())?;
        }
        self.list
            .Children()
            .map_err(|e| e.to_string())?
            .Append(&row)
            .map_err(|e| e.to_string())?;
        self.rows
            .lock()
            .map_err(|_| "Shortcut state unavailable")?
            .push(Row {
                key,
                choice,
                removed,
            });
        Ok(())
    }
    fn feedback(&self) {
        let message = self.validate().err().unwrap_or_default();
        let _ = self.error.SetText(&HSTRING::from(message));
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut keys = std::collections::HashSet::new();
        for row in self
            .rows
            .lock()
            .map_err(|_| "Shortcut state unavailable")?
            .iter()
        {
            if *row
                .removed
                .lock()
                .map_err(|_| "Shortcut state unavailable")?
            {
                continue;
            }
            let key = row
                .key
                .lock()
                .map_err(|_| "Shortcut state unavailable")?
                .clone();
            if key.is_empty() {
                return Err("Record a key for every shortcut, or remove the empty row.".into());
            }
            if !keys.insert(key.clone()) {
                return Err(format!(
                    "{key} is assigned twice. Record another key or remove a row."
                ));
            }
        }
        Ok(())
    }
    pub fn value(&self) -> Result<String, String> {
        let rows = self.rows.lock().map_err(|_| "Shortcut state unavailable")?;
        let mut lines = Vec::new();
        for row in rows.iter() {
            if *row
                .removed
                .lock()
                .map_err(|_| "Shortcut state unavailable")?
            {
                continue;
            }
            let key = row.key.lock().map_err(|_| "Shortcut state unavailable")?;
            let index = row.choice.SelectedIndex().map_err(|e| e.to_string())?;
            let (_, action) = ACTIONS
                .get(index as usize)
                .ok_or("Choose a pointer action")?;
            lines.push(format!("{key} = {action}"));
        }
        Ok(lines.join("\r"))
    }
}
