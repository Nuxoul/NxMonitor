use anyhow::Result;
use gpui_kit::component::Sizable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        ActiveTheme, Theme, ThemeMode,
        button::{Button, ButtonVariants},
    },
    *,
};
use nxmonitor::{
    capture::{Capture, CaptureState},
    options::Options,
};
use smol::Timer;
use std::time::{Duration, Instant};

#[path = "../native.rs"]
mod native;

fn main() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        // Explicit platform window geometry is a physical-boundary exception to rem layout.
        let bounds = Bounds::new(point(px(40.0), px(60.0)), size(px(380.0), px(580.0)));
        let result = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Hud::new(options, window, cx)),
        );
        if let Ok((handle, _)) = &result {
            let _ = handle.update(cx, |_, window, cx| {
                // Component's default Root background is opaque. Override only this HUD root.
                gpui_kit::base::Root::update(window, cx, |root, _, cx| {
                    root.style().background = Some(cx.theme().background.opacity(0.0).into());
                    cx.notify();
                });
            });
        }
        if let Err(error) = result {
            eprintln!("WINDOW_ERROR {error:#}");
            cx.quit();
        }
    });
    Ok(())
}

struct Hud {
    options: Options,
    capture: Option<Capture>,
    state: CaptureState,
    native: Option<native::OverlayWindow>,
    editing: bool,
    error: Option<SharedString>,
    started: Instant,
    _poll: Task<()>,
}
impl Hud {
    fn new(options: Options, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("NxMonitor — technical validation");
        let native = native::OverlayWindow::new(window);
        let mut error: Option<SharedString> =
            native.as_ref().err().map(|e| format!("{e:#}").into());
        if let Some(error) = &error {
            eprintln!("NATIVE_INIT_ERROR {error}");
            cx.quit();
        }
        let capture = if options.window_only {
            None
        } else {
            match Capture::start(
                options.presentmon.as_ref().unwrap(),
                options.pid.unwrap(),
                options.seconds,
            ) {
                Ok(capture) => Some(capture),
                Err(e) => {
                    error = Some(format!("{e:#}").into());
                    None
                }
            }
        };
        let task = cx.spawn_in(window, async move |view, cx| {
            loop {
                Timer::after(Duration::from_millis(100)).await;
                if view.update_in(cx, |this, _, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            options,
            capture,
            state: CaptureState::default(),
            native: native.ok(),
            editing: false,
            error,
            started: Instant::now(),
            _poll: task,
        }
    }
    fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        if self.options.english { en } else { zh }
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        if let Some(native) = &self.native {
            for command in native.drain_commands() {
                match command {
                    native::Command::Edit => self.toggle_editing(),
                    native::Command::Quit => {
                        cx.quit();
                        return;
                    }
                }
            }
        }
        if self
            .options
            .smoke_seconds
            .is_some_and(|s| self.started.elapsed() >= Duration::from_secs(s as u64))
        {
            eprintln!(
                "SMOKE_COMPLETE received={} error={:?}",
                self.state.received, self.error
            );
            cx.quit();
            return;
        }
        if let Some(capture) = &mut self.capture {
            match capture.poll() {
                Ok(state) => self.state = state,
                Err(error) => {
                    self.error = Some(format!("{error:#}").into());
                }
            }
        }
        cx.notify();
    }
    fn toggle_editing(&mut self) {
        let next = !self.editing;
        if let Some(native) = &mut self.native {
            match native.set_editing(next) {
                Ok(()) => self.editing = next,
                Err(e) => self.error = Some(format!("{e:#}").into()),
            }
        }
    }
    fn chart(&self, frame_time: bool, cx: &App) -> impl IntoElement {
        let samples = &self.state.snapshot.history_ms;
        let values: Vec<f64> = samples
            .iter()
            .rev()
            .step_by(2)
            .take(60)
            .map(|ms| if frame_time { *ms } else { 1000.0 / ms })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let max = values
            .iter()
            .copied()
            .fold(if frame_time { 33.3 } else { 60.0 }, f64::max);
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
            .children(values.into_iter().map(|value| {
                div()
                    .flex_1()
                    .min_w_0()
                    .h(relative((value / max).clamp(0.01, 1.0) as f32))
                    .bg(color.opacity(0.8))
            }))
    }
    fn metric(&self, label: &'static str, value: Option<f64>, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .gap_3()
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(div().font_family("Consolas").child(number(value)))
    }
}
impl Render for Hud {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(error) = self
            .error
            .clone()
            .or_else(|| self.state.error.clone().map(Into::into))
        {
            let diagnostic = format!("{}\n{}", error, self.state.diagnostics);
            let preview: String = diagnostic.chars().take(320).collect();
            return div().flex().flex_col().gap_3().p_4().size_full()
                .bg(cx.theme().background.opacity(0.9)).text_color(cx.theme().foreground)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("NxMonitor"))
                .child(div().text_color(cx.theme().danger).child(self.text("采集不可用", "Capture unavailable")))
                .child(div().text_sm().child(self.text("请检查目标 PID、PresentMon 路径或采集权限。不会自动提权。", "Check target PID, PresentMon path and capture permissions. No automatic elevation.")))
                .child(div().text_xs().child(preview))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(self.text("以上为诊断摘要；复制可获取完整信息。", "Diagnostic preview; copy for full details.")))
                .child(Button::new("copy-diagnostics").label(self.text("复制完整诊断", "Copy diagnostics")).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(diagnostic.clone()))))
                .child(Button::new("quit-error").label(self.text("退出", "Quit")).on_click(|_, _, cx| cx.quit()))
                .child(div().text_xs().child(format!("{} {}", self.text("编辑：", "Edit:"), self.native.as_ref().map_or("—", |n| n.edit_shortcut))))
                .into_any_element();
        }
        let stale = self
            .state
            .last_frame
            .is_some_and(|t| t.elapsed() > Duration::from_secs(2));
        let live =
            self.state.received > 0 && !stale && !self.state.finished && self.state.error.is_none();
        let current = if live {
            self.state.snapshot.current_fps
        } else {
            None
        };
        let ms = if live {
            self.state.snapshot.frame_ms
        } else {
            None
        };
        let status = if self.error.is_some() || self.state.error.is_some() {
            self.text("采集失败", "Capture failed")
        } else if self.options.window_only {
            self.text(
                "窗口验证 · 未连接采集器",
                "Window validation · no collector",
            )
        } else if self.state.finished {
            self.text("采集结束 · 保留统计", "Capture ended · statistics retained")
        } else if stale {
            self.text("暂无新帧 · 数据已过期", "No recent frames · stale")
        } else if live {
            self.text("正在采集 · 非注入", "Capturing · non-injected")
        } else {
            self.text("等待目标帧数据", "Waiting for target frames")
        };
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .size_full()
            .bg(cx
                .theme()
                .background
                .opacity(if self.options.graphy { 0.68 } else { 0.9 }))
            .text_color(cx.theme().foreground)
            .text_sm()
            .when(!self.options.graphy, |d| {
                d.rounded_lg().border_1().border_color(cx.theme().border)
            });
        panel = panel
            .child(
                div()
                    .id("drag-header")
                    .flex()
                    .justify_between()
                    .gap_3()
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
                            .child(self.text("技术验证", "TECH PREVIEW")),
                    ),
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
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(number(current)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.text("呈现 FPS", "presented FPS"))
                            .child(format!("{} ms", number(ms))),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "PID {} · {}",
                        self.options
                            .pid
                            .map_or_else(|| "—".into(), |p| p.to_string()),
                        if self.state.snapshot.swapchain.is_empty() {
                            "—"
                        } else {
                            &self.state.snapshot.swapchain
                        }
                    )),
            )
            .child(div().text_xs().child(self.text(
                "FPS · 最近 120 帧，隔帧显示",
                "FPS · last 120 frames, decimated",
            )))
            .child(self.chart(false, cx))
            .child(
                div()
                    .text_xs()
                    .child(self.text("帧时间 · 自动量程", "Frame time · auto scale")),
            )
            .child(self.chart(true, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.text("最近 30 秒", "ROLLING 30 SECONDS")),
                    )
                    .child(self.metric(
                        self.text("平均", "Average"),
                        self.state.snapshot.rolling.average_fps,
                        cx,
                    ))
                    .child(self.metric(
                        self.text("最低 / 1秒桶", "Minimum / 1s bucket"),
                        self.state.snapshot.rolling.min_fps,
                        cx,
                    ))
                    .child(self.metric(
                        self.text("最高 / 1秒桶", "Maximum / 1s bucket"),
                        self.state.snapshot.rolling.max_fps,
                        cx,
                    ))
                    .child(self.metric("1% low", self.state.snapshot.rolling.low_1, cx))
                    .child(self.metric("0.1% low", self.state.snapshot.rolling.low_01, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {} fps · {} {}",
                        self.text("会话平均", "Session average"),
                        number(self.state.snapshot.session.average_fps),
                        self.state.snapshot.session.samples,
                        self.text("帧", "frames")
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.text(
                        "— 表示样本不足或不可用",
                        "— means insufficient samples or unavailable",
                    )),
            )
            .child(div().text_xs().child(if self.editing {
                self.text("编辑模式 · 拖动标题定位", "Editing · drag header to move")
                    .to_owned()
            } else {
                format!(
                    "{} {}",
                    self.text("鼠标穿透 · 编辑：", "Pass-through · edit:"),
                    self.native.as_ref().map_or("—", |n| n.edit_shortcut)
                )
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {}",
                        self.text("退出：", "Quit:"),
                        self.native.as_ref().map_or("—", |n| n.quit_shortcut)
                    )),
            );
        if self.editing {
            panel = panel.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("lock")
                            .small()
                            .label(self.text("锁定并穿透", "Lock HUD"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_editing();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("quit")
                            .small()
                            .ghost()
                            .label(self.text("退出", "Quit"))
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            );
        }
        panel.into_any_element()
    }
}
impl Drop for Hud {
    fn drop(&mut self) {
        eprintln!(
            "CAPTURE_SUMMARY received={} rejected={} error={:?}",
            self.state.received, self.state.rejected, self.state.error
        );
        eprintln!("HUD_DROP: stopping owned capture session");
        self.capture.take();
    }
}
fn number(value: Option<f64>) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:.1}"))
}
