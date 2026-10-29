use super::*;
use clickless_core::{
    KeyEvent, Phase,
    practice::{PRACTICE_VERSION, Practice, Step},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Screen {
    Home,
    Settings,
    Practice,
}

impl Settings {
    pub(super) fn start_practice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending || self.dirty() {
            return;
        }
        let config = self.editor.draft();
        self.practice = Practice::with_grid(config.settings.leader, config.grid.clone());
        if config.practice_completed_version < PRACTICE_VERSION {
            self.practice
                .choose_activation_key(&self.defaults.activation_keys);
        }
        self.practice_started = std::time::Instant::now();
        self.practice_modifier_held = false;
        self.screen = Screen::Practice;
        self.capture = None;
        window.focus(&self.capture_focus, cx);
        cx.notify();
    }

    pub(super) fn practice_key(
        &mut self,
        key: &str,
        phase: Phase,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.capture_focus.is_focused(window) || self.pending {
            return;
        }
        let name = match key {
            "escape" => "esc",
            "control" => "controlleft",
            "shift" => "shiftleft",
            other => other,
        };
        if let Ok(key) = clickless_config::parse_logical_key(name) {
            cx.stop_propagation();
            self.practice.key(
                KeyEvent::new(key, phase),
                self.practice_started.elapsed().as_millis() as u64,
            );
            cx.notify();
        }
    }

    pub(super) fn dashboard(&mut self, p: theme::Palette, cx: &mut Context<Self>) -> AnyElement {
        let config = self.editor.draft();
        let home = clickless_core::home::HomeView::build(
            config.enabled,
            config.settings.leader,
            config.practice_completed_version >= PRACTICE_VERSION,
        );
        let practice = self.screen == Screen::Practice;
        let mut content = div()
            .id("dashboard-scroll")
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .text_size(px(26.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if practice {
                        "Practice"
                    } else {
                        "Pointer control"
                    }),
            )
            .child(div().mt_3().text_color(p.secondary).child(if practice {
                "Local keyboard lesson. Your real pointer stays still.".to_string()
            } else {
                home.status
            }))
            .child(
                div()
                    .mt_6()
                    .max_w(px(680.))
                    .text_size(px(16.))
                    .child(if practice {
                        self.practice.instruction()
                    } else {
                        home.explanation
                    }),
            );
        if let Some(error) = self.error.as_ref() {
            content = content.child(
                div()
                    .id("dashboard-error")
                    .mt_4()
                    .role(Role::Alert)
                    .aria_label(error.clone())
                    .text_color(p.error)
                    .child(error.clone()),
            );
        }
        if practice {
            content = content.child(
                div()
                    .id("practice-status")
                    .mt_4()
                    .role(Role::Status)
                    .aria_label(self.practice.status())
                    .child(self.practice.status()),
            );
            if let Some(frame) = self.practice.grid_overlay() {
                let columns = if config.grid.dense {
                    config.grid.column_keys.len()
                } else {
                    config.grid.cols as usize
                };
                let columns = columns.max(1);
                content = content.child(div().mt_6().flex().flex_col().gap_2().children(
                    frame.cells.chunks(columns).enumerate().map(|(row, cells)| {
                        div()
                            .id(("lesson-row", row))
                            .flex()
                            .gap_2()
                            .children(cells.iter().map(|cell| {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .py_2()
                                    .text_center()
                                    .border_1()
                                    .border_color(p.border)
                                    .bg(p.surface)
                                    .child(cell.label.clone())
                            }))
                    }),
                ));
            }
            let done = self.practice.step() == Step::Done;
            content = content.child(
                div()
                    .mt_6()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        Button::new("restart-practice")
                            .label("Restart")
                            .disabled(self.pending)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_practice(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("leave-practice")
                            .label("Back to Home")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.practice.cancel();
                                this.screen = Screen::Home;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("finish-practice")
                            .primary()
                            .label("Finish and save")
                            .disabled(!done || self.pending)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let mut config = this.editor.draft().clone();
                                config.settings.leader = this.practice.chosen_leader();
                                config.practice_completed_version = PRACTICE_VERSION;
                                this.values = values(&config);
                                if let Some(input) = this.inputs.get("leader") {
                                    input.update(cx, |input, cx| {
                                        input.set_value(
                                            clickless_config::logical_key_name(
                                                config.settings.leader,
                                            ),
                                            window,
                                            cx,
                                        )
                                    });
                                }
                                this.editor = SettingsEditor::new(config);
                                this.unsaved = true;
                                this.submit(true, cx);
                            })),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .mt_6()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        Button::new("use-pointer-control")
                            .primary()
                            .label("Use pointer control")
                            .disabled(
                                self.pending
                                    || self.dirty()
                                    || !config.enabled
                                    || self.apply.is_none(),
                            )
                            .on_click(cx.listener(|_, _, window, _| window.minimize_window())),
                    )
                    .child(
                        Button::new("start-practice")
                            .primary()
                            .label("Start practice")
                            .disabled(self.pending || self.dirty())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_practice(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("open-settings")
                            .label("Open Settings")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.screen = Screen::Settings;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("toggle-enabled")
                            .label(if config.enabled {
                                "Pause pointer control"
                            } else {
                                "Resume pointer control"
                            })
                            .disabled(self.pending || self.dirty() || self.apply.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let enabled = !this.editor.draft().enabled;
                                this.change("enabled", DraftValue::Flag(enabled), cx);
                                this.submit(false, cx);
                            })),
                    ),
            );
            content = content.child(div().mt_6().text_color(p.secondary).child(
                if self.apply.is_none() {
                    "Standalone window. Open from running Clickless for live controls."
                } else {
                    "Keyboard capture waits while this window has focus. Use pointer control minimizes Home so your activation key works. Changes affect the running session; Save makes them permanent."
                },
            ));
            content = content.child(
                Button::new("quit-clickless")
                    .ghost()
                    .label(if self.apply.is_some() {
                        "Quit Clickless"
                    } else {
                        "Close window"
                    })
                    .disabled(self.pending)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.allow_close(cx) {
                            return;
                        }
                        if std::env::args().any(|arg| arg == "--runtime" || arg == "--runtime-unix")
                        {
                            use std::io::Write;
                            let mut output = std::io::stdout().lock();
                            if writeln!(output, "quit")
                                .and_then(|_| output.flush())
                                .is_err()
                            {
                                return;
                            }
                        }
                        cx.quit();
                    })),
            );
            if self.dirty() {
                content = content.child(
                    div()
                        .mt_3()
                        .child("Save or discard Settings changes before starting practice."),
                );
            }
        }
        content.into_any_element()
    }
}
