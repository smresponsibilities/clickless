//! Settings presentation only. Backends own runtime Apply and input capture.
mod theme;

use clickless_config::editor::{SettingsEditor, fields_from_config};
use clickless_config::editor_controls::{
    CHOICE_OPTIONS, DraftValue, draft_value, fields_with_draft,
};
use clickless_config::settings_model::SETTINGS;
use clickless_config::{Config, SettingDescriptor, SettingKind, SettingsPage};
use gpui_kit::component::{
    ActiveTheme, Disableable, Root, Selectable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    switch::Switch,
};
use gpui_kit::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type ApplyCallback = Box<dyn FnMut(&Config) -> Result<(), String> + Send>;

/// Runs a single Settings window. None means standalone editing and atomic Save.
pub fn run(seed: Config, path: PathBuf, apply: Option<ApplyCallback>) -> Result<(), String> {
    let failure = Arc::new(Mutex::new(None));
    let startup_failure = failure.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply(cx.theme().is_dark(), None, cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(760.)),
                    cx,
                ))),
                window_min_size: Some(size(px(640.), px(480.))),
                ..Default::default()
            };
            if let Err(error) = cx.open_window(options, move |window, cx| {
                window.set_window_title("Clickless Settings");
                let view = cx.new(|cx| Settings::new(seed, path, apply, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            }) {
                *startup_failure.lock().expect("startup state poisoned") = Some(error.to_string());
                cx.quit();
            }
            cx.activate(true);
        });
    failure
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .map_or(Ok(()), Err)
}

struct Settings {
    editor: SettingsEditor,
    values: BTreeMap<&'static str, DraftValue>,
    baseline: BTreeMap<&'static str, DraftValue>,
    inputs: BTreeMap<&'static str, Entity<InputState>>,
    bindings: Entity<TextareaState>,
    search: Entity<InputState>,
    page: SettingsPage,
    path: PathBuf,
    apply: Option<Arc<Mutex<ApplyCallback>>>,
    pending: bool,
    unsaved: bool,
    error: Option<String>,
    invalid: bool,
    message: String,
    theme_override: Option<bool>,
    capture: Option<&'static str>,
    capture_focus: FocusHandle,
    subscriptions: Vec<Subscription>,
}

fn values(config: &Config) -> BTreeMap<&'static str, DraftValue> {
    let fields = fields_from_config(config);
    SETTINGS
        .iter()
        .filter(|d| d.page != SettingsPage::About)
        .map(|d| {
            let raw = draft_value(&fields, d.key);
            let value = match d.kind {
                SettingKind::Toggle => DraftValue::Flag(raw == "On"),
                SettingKind::Choice => DraftValue::Choice(usize::from(raw == "simple")),
                _ => DraftValue::Text(raw),
            };
            (d.key, value)
        })
        .collect()
}

impl Settings {
    fn new(
        seed: Config,
        path: PathBuf,
        apply: Option<ApplyCallback>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let values = values(&seed);
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (&key, value) in &values {
            if key == "mouse_bindings" {
                continue;
            }
            if let DraftValue::Text(text) = value {
                let input = cx.new(|cx| InputState::new(window, cx).default_value(text.clone()));
                subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.change(
                                key,
                                DraftValue::Text(input.read(cx).value().to_string()),
                                cx,
                            );
                        }
                    },
                ));
                inputs.insert(key, input);
            }
        }
        let raw = draft_value(&fields_from_config(&seed), "mouse_bindings");
        let bindings = cx.new(|cx| TextareaState::new(window, cx).default_value(raw));
        subscriptions.push(
            cx.subscribe_in(&bindings, window, |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.change(
                        "mouse_bindings",
                        DraftValue::Text(input.read(cx).value().to_string()),
                        cx,
                    );
                }
            }),
        );
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        subscriptions
            .push(cx.subscribe_in(&search, window, |_, _, _: &InputEvent, _, cx| cx.notify()));
        subscriptions.push(cx.observe_window_appearance(window, |this, window, cx| {
            if this.theme_override.is_none() {
                theme::apply(
                    gpui_kit::component::ThemeMode::from(window.appearance()).is_dark(),
                    Some(window),
                    cx,
                );
                cx.notify();
            }
        }));
        Self {
            editor: SettingsEditor::new(seed),
            baseline: values.clone(),
            values,
            inputs,
            bindings,
            search,
            page: SettingsPage::General,
            path,
            apply: apply.map(|a| Arc::new(Mutex::new(a))),
            pending: false,
            unsaved: false,
            error: None,
            invalid: false,
            message: "No changes".into(),
            theme_override: None,
            capture: None,
            capture_focus: cx.focus_handle(),
            subscriptions,
        }
    }

    fn change(&mut self, key: &'static str, value: DraftValue, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.values.insert(key, value);
        self.validate();
        self.message = if self.dirty() {
            "Unsaved changes"
        } else if self.unsaved {
            "Applied. Not saved."
        } else {
            "No changes"
        }
        .into();
        cx.notify();
    }

    fn dirty(&self) -> bool {
        self.values != self.baseline
    }

    fn record_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending || !self.capture_focus.is_focused(window) {
            return;
        }
        let Some(target) = self.capture else {
            return;
        };
        cx.stop_propagation();
        let key = event.keystroke.key.as_str();
        if key == "escape" || key == "esc" {
            self.capture = None;
            if target == "leader" {
                window.focus(&self.inputs["leader"].read(cx).focus_handle(cx), cx);
            } else {
                window.focus(&self.bindings.read(cx).focus_handle(cx), cx);
            }
            self.message = "Key recording cancelled".into();
            cx.notify();
            return;
        }
        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || event.keystroke.modifiers.shift
            || event.keystroke.modifiers.platform
        {
            self.message =
                "Record one key. Type modifier names such as controlleft directly.".into();
            cx.notify();
            return;
        }
        match clickless_config::parse_logical_key(key) {
            Ok(logical) => {
                let name = clickless_config::logical_key_name(logical);
                let raw = if target == "leader" {
                    name.to_string()
                } else {
                    let current = self.bindings.read(cx).value();
                    format!("{current}\n{name} = click_left")
                };
                self.capture = None;
                if target == "leader" {
                    let input = &self.inputs["leader"];
                    input.update(cx, |input, cx| input.set_value(raw.clone(), window, cx));
                    window.focus(&input.read(cx).focus_handle(cx), cx);
                } else {
                    self.bindings
                        .update(cx, |input, cx| input.set_value(raw.clone(), window, cx));
                    window.focus(&self.bindings.read(cx).focus_handle(cx), cx);
                }
                self.change(target, DraftValue::Text(raw), cx);
            }
            Err(_) => {
                self.message = "Key unsupported. Try another key, or press Esc to cancel.".into();
                cx.notify();
            }
        }
    }

    fn validate(&mut self) {
        self.error = fields_with_draft(&fields_from_config(self.editor.applied()), &self.values)
            .and_then(|fields| self.editor.edit(&fields))
            .err();
        self.invalid = self.error.is_some();
    }

    fn restore(&mut self, reset: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.capture = None;
        let config = if reset {
            Config::default()
        } else {
            self.editor.applied().clone()
        };
        self.values = if reset {
            values(&config)
        } else {
            self.baseline.clone()
        };
        for (&key, input) in &self.inputs {
            if let Some(DraftValue::Text(raw)) = self.values.get(key) {
                input.update(cx, |input, cx| input.set_value(raw.clone(), window, cx));
            }
        }
        if let Some(DraftValue::Text(raw)) = self.values.get("mouse_bindings") {
            self.bindings
                .update(cx, |input, cx| input.set_value(raw.clone(), window, cx));
        }
        self.validate();
        self.message = if reset {
            "Defaults restored to draft. Apply or Save to keep them."
        } else {
            "Changes discarded"
        }
        .into();
        cx.notify();
    }

    fn submit(&mut self, save: bool, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.capture = None;
        self.validate();
        if self.error.is_some() {
            cx.notify();
            return;
        }
        if !save && self.apply.is_none() {
            return;
        }
        let config = self.editor.draft().clone();
        let path = self.path.clone();
        let callback = self.apply.clone();
        let standalone = callback.is_none();
        self.pending = true;
        self.message = if save { "Saving…" } else { "Applying…" }.into();
        cx.notify();
        let job = cx.background_executor().spawn(async move {
            let apply_result = match callback {
                Some(callback) => callback
                    .lock()
                    .map_err(|e| e.to_string())
                    .and_then(|mut apply| apply(&config)),
                None => Ok(()),
            };
            let save_result = if apply_result.is_ok() && save {
                config.save_to_file(path).map_err(|e| e.to_string())
            } else {
                Ok(())
            };
            (apply_result, save_result)
        });
        cx.spawn(async move |this, cx| {
            let (applied, saved) = job.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                if let Err(error) = applied {
                    this.error = Some(error);
                    this.message =
                        "Apply was not confirmed. Draft retained; retry or reopen Settings.".into();
                } else if standalone && saved.is_err() {
                    this.error = saved.err();
                    this.message =
                        "Save failed. Draft retained; correct file access and retry.".into();
                } else {
                    match this.editor.apply() {
                        Ok(_) => {
                            this.baseline = this.values.clone();
                            this.unsaved = !save || saved.is_err();
                        }
                        Err(error) => {
                            this.error = Some(error);
                            cx.notify();
                            return;
                        }
                    }
                    this.error = saved.err();
                    this.message = if this.error.is_some() {
                        "Applied, but Save failed. Correct the file access and retry."
                    } else if save {
                        "Saved"
                    } else {
                        "Applied. Not saved."
                    }
                    .into();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn control(&self, d: &SettingDescriptor, cx: &mut Context<Self>) -> AnyElement {
        let key = d.key;
        match self.values.get(key) {
            Some(DraftValue::Flag(checked)) => Switch::new(key)
                .checked(*checked)
                .accessibility_label(d.title)
                .disabled(self.pending)
                .on_click(cx.listener(move |this, checked, _, cx| {
                    this.change(key, DraftValue::Flag(*checked), cx);
                }))
                .into_any_element(),
            Some(DraftValue::Choice(selected)) => div()
                .flex()
                .gap_2()
                .children(
                    CHOICE_OPTIONS
                        .iter()
                        .enumerate()
                        .map(|(index, &(label, _))| {
                            Button::new((key, index))
                                .label(label)
                                .selected(*selected == index)
                                .disabled(self.pending)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.change(key, DraftValue::Choice(index), cx)
                                }))
                        }),
                )
                .into_any_element(),
            _ if key == "mouse_bindings" => Textarea::new(&self.bindings)
                .aria_label(d.title)
                .h(px(260.))
                .w_full()
                .disabled(self.pending)
                .into_any_element(),
            _ => Input::new(self.inputs.get(key).expect("descriptor has stable input"))
                .id(key)
                .aria_label(d.title)
                .w_full()
                .disabled(self.pending)
                .into_any_element(),
        }
    }

    fn row(
        &self,
        d: &'static SettingDescriptor,
        narrow: bool,
        palette: theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_w_0()
            .child(div().font_weight(FontWeight::MEDIUM).child(d.title))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.secondary)
                    .child(d.description),
            );
        let mut row = div()
            .flex()
            .gap_4()
            .py_4()
            .border_b_1()
            .border_color(palette.border);
        if narrow || d.key == "mouse_bindings" {
            row = row.flex_col();
        } else {
            row = row.items_center();
        }
        let mut control = self.control(d, cx);
        if d.key == "leader" || d.key == "mouse_bindings" {
            let key = d.key;
            control = div().flex().flex_col().gap_2().child(control)
                .child(Button::new((key, 1usize)).label(if key == "leader" { "Record key" } else { "Record click shortcut" }).disabled(self.pending)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.capture = Some(key);
                        this.message = "Press one key. Esc cancels. Type left/right modifier names directly.".into();
                        window.focus(&this.capture_focus, cx);
                        cx.notify();
                    })))
                .into_any_element();
        }
        let control = if narrow || d.key == "mouse_bindings" {
            div().w_full().child(control)
        } else {
            div().w(px(220.)).flex_shrink_0().child(control)
        };
        row.child(text).child(control).into_any_element()
    }
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.subscriptions;
        let dark = cx.theme().is_dark();
        let p = theme::Palette::new(dark);
        let narrow = window.bounds().size.width < px(840.);
        let query = self.search.read(cx).value().to_lowercase();
        let matches: Vec<_> = SETTINGS
            .iter()
            .filter(|d| {
                d.page != SettingsPage::About
                    && if query.trim().is_empty() {
                        d.page == self.page
                    } else {
                        format!("{} {} {} {}", d.title, d.description, d.key, d.page.title())
                            .to_lowercase()
                            .contains(query.trim())
                    }
            })
            .collect();
        let header = if query.trim().is_empty() {
            self.page.title().to_string()
        } else {
            "Search results".into()
        };
        let subtitle = match self.page {
            SettingsPage::General => "Make pointer control work with your keyboard.",
            SettingsPage::Movement => "Tune movement speed, acceleration, and scrolling.",
            SettingsPage::Grid => "Choose targets, then make precise adjustments.",
            SettingsPage::Shortcuts => "Choose the keys used while pointer mode is active.",
            SettingsPage::Appearance => "Keep grid labels readable on your displays.",
            SettingsPage::About => "Keyboard-driven pointer control, built with Rust.",
        };
        let rail = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(if narrow { 168. } else { 208. }))
            .bg(p.rail)
            .p_4()
            .gap_2()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .px_2()
                    .py_3()
                    .child("Clickless"),
            )
            .children(
                SettingsPage::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, page)| {
                        Button::new(("page", index))
                            .ghost()
                            .icon(
                                [
                                    gpui_kit::assets::IconName::Settings,
                                    gpui_kit::assets::IconName::MousePointer,
                                    gpui_kit::assets::IconName::Grid2x2,
                                    gpui_kit::assets::IconName::Keyboard,
                                    gpui_kit::assets::IconName::Palette,
                                    gpui_kit::assets::IconName::Info,
                                ][index],
                            )
                            .label(page.title())
                            .selected(self.page == page && query.is_empty())
                            .w_full()
                            .h(px(36.))
                            .justify_start()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.page = page;
                                this.search
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                                cx.notify();
                            }))
                    }),
            )
            .child(div().flex_1());
        let mut body = div()
            .id("settings-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_6()
            .py_5()
            .child(
                div()
                    .max_w(px(680.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(26.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(header),
                    )
                    .child(
                        div()
                            .text_color(p.secondary)
                            .text_size(px(13.))
                            .child(subtitle),
                    ),
            )
            .children(self.error.as_ref().map(|error| {
                div()
                    .id("error-summary")
                    .mt_4()
                    .p_3()
                    .border_1()
                    .rounded_md()
                    .border_color(p.error)
                    .text_color(p.error)
                    .role(Role::Alert)
                    .aria_label(error.clone())
                    .child(format!("Cannot finish: {error}"))
            }));
        if self.page == SettingsPage::About && query.is_empty() {
            body = body.child(
                div()
                    .mt_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(format!("Clickless {}", env!("CARGO_PKG_VERSION")))
                    .child(
                        "Hold your activation key to control the pointer. Release it to return to typing.",
                    )
                    .child(Button::new("support").label("Help and source")
                        .on_click(|_, _, cx| cx.open_url("https://github.com/smresponsibilities/clickless")))
                    .child("License: MIT OR Apache-2.0")
                    .child(Button::new("licenses").label("Copy license notices")
                        .on_click(|_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(
                            concat!(include_str!("../LICENSE-MIT"), "\n", include_str!("../LICENSE-APACHE"),
                                "\nGPUI uses Lucide icons:\n", include_str!("../LICENSE-LUCIDE")).to_string()
                        ))))
                    .child(format!("Configuration: {}", self.path.display()))
                    .child(if self.apply.is_some() {
                        "Connected to running Clickless"
                    } else {
                        "Standalone editor. Save takes effect at the next launch."
                    }),
            );
        } else if matches.is_empty() {
            body = body.child(
                div()
                    .mt_6()
                    .child("No settings match. Try speed, opacity, or activation."),
            );
        } else {
            body = body.child(
                div()
                    .max_w(px(680.))
                    .mt_4()
                    .children(matches.into_iter().map(|d| self.row(d, narrow, p, cx))),
            );
        }
        let footer = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(p.border)
            .bg(p.page)
            .child(
                div()
                    .flex_1()
                    .min_w(px(150.))
                    .text_size(px(13.))
                    .text_color(p.secondary)
                    .child(self.message.clone()),
            )
            .child(
                Button::new("cancel")
                    .label("Discard")
                    .disabled(self.pending || !self.dirty())
                    .on_click(cx.listener(|this, _, window, cx| this.restore(false, window, cx))),
            )
            .child(
                Button::new("apply")
                    .label("Apply")
                    .disabled(self.pending || self.invalid || !self.dirty() || self.apply.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.submit(false, cx))),
            )
            .child(
                Button::new("save")
                    .primary()
                    .label("Save")
                    .disabled(self.pending || self.invalid || !(self.dirty() || self.unsaved))
                    .on_click(cx.listener(|this, _, _, cx| this.submit(true, cx))),
            );
        div()
            .id("settings-root")
            .track_focus(&self.capture_focus)
            .on_key_down(cx.listener(|this, event, window, cx| this.record_key(event, window, cx)))
            .size_full()
            .flex()
            .bg(p.page)
            .text_color(p.text)
            .font_family(cx.theme().font_family.clone())
            .text_size(px(14.))
            .child(rail)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_3()
                            .px_6()
                            .py_4()
                            .border_b_1()
                            .border_color(p.border)
                            .child(
                                Input::new(&self.search)
                                    .id("search")
                                    .aria_label("Search settings")
                                    .w(px(if narrow { 200. } else { 280. })),
                            )
                            .child(div().flex_1())
                            .children(
                                [
                                    ("System", None),
                                    ("Light", Some(false)),
                                    ("Dark", Some(true)),
                                ]
                                .into_iter()
                                .map(|(label, mode)| {
                                    Button::new(label)
                                        .ghost()
                                        .label(label)
                                        .selected(self.theme_override == mode)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.theme_override = mode;
                                            let dark = mode.unwrap_or_else(|| {
                                                gpui_kit::component::ThemeMode::from(
                                                    window.appearance(),
                                                )
                                                .is_dark()
                                            });
                                            theme::apply(dark, Some(window), cx);
                                            cx.notify();
                                        }))
                                }),
                            )
                            .child(
                                Button::new("reset")
                                    .ghost()
                                    .label("Reset")
                                    .disabled(self.pending)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.restore(true, window, cx)
                                    })),
                            ),
                    )
                    .child(body)
                    .child(footer),
            )
    }
}
