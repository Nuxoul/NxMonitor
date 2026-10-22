use super::{
    memory,
    model::{DesktopModel, open_settings},
    runtime::Phase,
};
use gpui_kit::{
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::time::Duration;
#[path = "../native.rs"]
mod native;

pub struct DesktopHud {
    model: Entity<DesktopModel>,
    native: Option<native::OverlayWindow>,
    editing: bool,
    native_error: Option<SharedString>,
    _poll: Task<()>,
}
impl DesktopHud {
    pub fn new(model: Entity<DesktopModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("NxMonitor — HUD");
        let native = native::OverlayWindow::new(window);
        let native_error = native.as_ref().err().map(|e| format!("{e:#}").into());
        let task = cx.spawn_in(window, async move |view, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(150)).await;
                if view
                    .update(cx, |this, cx| {
                        if let Some(native) = &this.native {
                            for command in native.drain_commands() {
                                match command {
                                    native::Command::Edit => this.toggle(),
                                    native::Command::Quit => cx.quit(),
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            model,
            native: native.ok(),
            editing: native_error.is_some(),
            native_error,
            _poll: task,
        }
    }
    fn toggle(&mut self) {
        let next = !self.editing;
        if let Some(native) = &mut self.native {
            match native.set_editing(next) {
                Ok(()) => self.editing = next,
                Err(e) => self.native_error = Some(format!("{e:#}").into()),
            }
        }
    }
    fn chart(&self, history: &[f64], frame_time: bool, cx: &App) -> impl IntoElement {
        let values: Vec<_> = history
            .iter()
            .rev()
            .step_by(2)
            .take(60)
            .map(|ms| if frame_time { *ms } else { 1000. / ms })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let max = values
            .iter()
            .copied()
            .fold(if frame_time { 33.3 } else { 60. }, f64::max);
        let color = if frame_time {
            cx.theme().warning
        } else {
            cx.theme().success
        };
        div()
            .flex()
            .items_end()
            .gap_0p5()
            .h_12()
            .w_full()
            .overflow_hidden()
            .children(values.into_iter().map(|v| {
                div()
                    .flex_1()
                    .min_w_0()
                    .h(relative((v / max).clamp(0.01, 1.) as f32))
                    .bg(color.opacity(0.85))
            }))
    }
    fn memory_chart(
        history: &std::collections::VecDeque<Option<u64>>,
        max: u64,
        color: Hsla,
    ) -> impl IntoElement {
        let values: Vec<_> = history.iter().copied().collect();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                // Canvas uses resolved physical geometry; ordinary panel layout remains rem-based.
                let mut segment = Vec::new();
                let paint = |points: &mut Vec<Point<Pixels>>, window: &mut Window| {
                    if points.len() >= 2 {
                        let mut path = PathBuilder::stroke(px(1.));
                        path.move_to(points[0]);
                        for point in &points[1..] {
                            path.line_to(*point);
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                    points.clear();
                };
                for (ix, value) in values.iter().enumerate() {
                    if let Some(value) = value {
                        let x = bounds.origin.x
                            + bounds.size.width * (ix as f32 / (memory::HISTORY_LIMIT - 1) as f32);
                        let y = bounds.origin.y
                            + bounds.size.height
                                * (1. - (*value as f64 / max.max(1) as f64).clamp(0., 1.) as f32);
                        segment.push(point(x, y));
                    } else {
                        paint(&mut segment, window);
                    }
                }
                paint(&mut segment, window);
            },
        )
        .w_full()
        .h_6()
    }
    fn memory_panel(
        value: &memory::Snapshot,
        target_id: Option<(u32, u64)>,
        english: bool,
        cx: &App,
    ) -> impl IntoElement {
        let t = |zh, en| if english { en } else { zh };
        let fresh = value.fresh();
        let system: SharedString = if !fresh {
            "—".into()
        } else {
            match value.system {
                Ok(s) => format!(
                    "{:.2} / {:.2} GiB",
                    memory::gib(s.used_bytes),
                    memory::gib(s.total_bytes)
                )
                .into(),
                Err(error) => error.label(english).into(),
            }
        };
        let process: SharedString = if target_id.is_none() {
            t("未选择目标", "No target selected").into()
        } else if !fresh || value.target_id != target_id {
            "—".into()
        } else {
            match value.working_set_bytes {
                Ok(bytes) => format!("{:.1} MiB", memory::mib(bytes)).into(),
                Err(error) => error.label(english).into(),
            }
        };
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_1()
            .border_t_1()
            .border_color(cx.theme().border)
            .pt_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        "内存 · 1秒采样 · 最多60点",
                        "Memory · 1s samples · up to 60 points",
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .child(t("系统已用/总物理", "System used/total physical"))
                    .child(system),
            );
        if fresh && let Ok(physical) = value.system {
            panel = panel.child(Self::memory_chart(
                &value.system_history,
                physical.total_bytes,
                cx.theme().info,
            ));
        }
        panel = panel.child(
            div()
                .flex()
                .justify_between()
                .gap_2()
                .child(t("目标工作集", "Target working set"))
                .child(process),
        );
        if fresh && value.target_id == target_id && target_id.is_some() {
            let max = value
                .process_history
                .iter()
                .flatten()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1);
            panel = panel.child(Self::memory_chart(
                &value.process_history,
                max,
                cx.theme().warning,
            ));
        }
        panel.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if fresh {
                    t(
                        "系统线：总物理量程；进程线：自动量程",
                        "System: physical capacity; process: auto scale",
                    )
                } else {
                    t(
                        "等待内存采样或数据已过期",
                        "Waiting for memory sample or stale data",
                    )
                }),
        )
    }
    fn metric(label: &'static str, value: Option<f64>, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .gap_3()
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(div().font_family("Consolas").child(number(value)))
    }
}
impl Render for DesktopHud {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let settings = model.settings.clone();
        let snapshot = model.snapshot();
        let t = |zh, en| if settings.english { en } else { zh };
        let stale = snapshot
            .capture
            .last_frame
            .is_none_or(|last| last.elapsed() > Duration::from_secs(2));
        let live = snapshot.phase == Phase::Capturing && !stale && snapshot.capture.error.is_none();
        let s = &snapshot.capture.snapshot;
        let status = match snapshot.phase {
            Phase::Waiting => t(
                "等待目标 · 在设置中选择",
                "Waiting for target · open settings",
            ),
            Phase::Switching => t("切换目标 · 重建会话", "Switching target · new session"),
            Phase::Capturing => {
                if live {
                    t("呈现帧 · 非注入采集", "Presented frames · non-injected")
                } else {
                    t(
                        "等待新帧 · 当前数据不可用",
                        "Waiting for frames · unavailable",
                    )
                }
            }
            Phase::Completed => t(
                "会话结束 · 可在设置中重新采集",
                "Session ended · restart in settings",
            ),
            Phase::Failed => t(
                "采集不可用 · 请查看设置",
                "Capture unavailable · see settings",
            ),
        };
        let target: SharedString = snapshot.target.as_ref().map_or_else(
            || t("未选择目标", "No target selected").into(),
            |p| format!("{} · {}", p.name, p.pid).into(),
        );
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .size_full()
            .bg(cx
                .theme()
                .background
                .opacity(if settings.graphy { 0.68 } else { 0.9 }))
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(
                div()
                    .id("desktop-hud-drag")
                    .flex()
                    .justify_between()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            if this.editing
                                && let Some(native) = &this.native
                            {
                                native.start_move();
                            }
                        }),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("NxMonitor"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                if settings.mode == nxmonitor::config::SelectionMode::Automatic {
                                    t("自动", "AUTO")
                                } else {
                                    t("锁定", "LOCKED")
                                },
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(target),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if live {
                        cx.theme().success
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(status),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_3()
                    .child(
                        div()
                            .text_3xl()
                            .font_family("Consolas")
                            .child(number(if live { s.current_fps } else { None })),
                    )
                    .child(div().text_xs().child(format!(
                        "fps   {} ms",
                        number(if live { s.frame_ms } else { None })
                    ))),
            )
            .child(div().text_xs().child("fps"))
            .child(self.chart(&s.history_ms, false, cx))
            .child(div().text_xs().child(t("帧时间", "Frame time")))
            .child(self.chart(&s.history_ms, true, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        "最近30秒 · 当前交换链",
                        "Rolling 30s · selected swapchain",
                    )),
            )
            .child(Self::metric(
                t("平均", "Average"),
                s.rolling.average_fps,
                cx,
            ))
            .child(Self::metric(
                t("最低 / 秒", "Minimum / second"),
                s.rolling.min_fps,
                cx,
            ))
            .child(Self::metric(
                t("最高 / 秒", "Maximum / second"),
                s.rolling.max_fps,
                cx,
            ))
            .child(Self::metric("1% low", s.rolling.low_1, cx))
            .child(Self::metric("0.1% low", s.rolling.low_01, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {} fps · {} {}",
                        t("会话平均", "Session average"),
                        number(s.session.average_fps),
                        s.session.samples,
                        t("帧", "frames")
                    )),
            )
            .child(Self::memory_panel(
                &snapshot.memory,
                snapshot
                    .target
                    .as_ref()
                    .map(nxmonitor::target::ProcessIdentity::id),
                settings.english,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if snapshot.device.fresh() {
                        match &snapshot.device.display {
                            Ok(display) => display.summary(settings.english),
                            Err(error) => error.label(settings.english).into(),
                        }
                    } else {
                        "—".into()
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{}: {}",
                        t("目标客户区", "Target client"),
                        snapshot.device.window_summary(settings.english)
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {}",
                        t("编辑：", "Edit:"),
                        self.native.as_ref().map_or("—", |n| n.edit_shortcut)
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {}",
                        t("退出：", "Quit:"),
                        self.native.as_ref().map_or("—", |n| n.quit_shortcut)
                    )),
            );
        if self.editing {
            panel = panel.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("open-settings")
                            .small()
                            .label(t("设置…", "Settings…"))
                            .on_click(cx.listener(|this, _, _, cx| open_settings(&this.model, cx))),
                    )
                    .child(
                        Button::new("lock-hud")
                            .small()
                            .label(t("穿透", "Pass-through"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("quit-hud")
                            .small()
                            .ghost()
                            .label(t("退出", "Quit"))
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            );
        }
        if let Some(error) = self.native_error.clone() {
            panel = panel.child(div().text_xs().text_color(cx.theme().danger).child(error));
        }
        panel
    }
}
fn number(value: Option<f64>) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:.1}"))
}
