use super::{
    model::DesktopModel,
    processes,
    runtime::{Command, Phase},
};
use gpui_kit::{
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        radio::RadioGroup,
        scroll::ScrollableElement,
        switch::Switch,
    },
    *,
};
use nxmonitor::{
    config::SelectionMode,
    target::{BlacklistRule, ProcessIdentity},
};
use std::time::Duration;

pub struct SettingsView {
    model: Entity<DesktopModel>,
    helper: Entity<InputState>,
    filter: Entity<InputState>,
    rule: Entity<InputState>,
    rule_path: bool,
    pending: Option<ProcessIdentity>,
    notice: Option<SharedString>,
    failed: bool,
    _subscriptions: Vec<Subscription>,
    _poll: Task<()>,
}
impl SettingsView {
    pub fn new(model: Entity<DesktopModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let helper_value = model.read(cx).settings.presentmon_path.clone();
        let helper = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(helper_value, window, cx);
            state
        });
        let filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("进程名称 / PID / Process name"));
        let rule =
            cx.new(|cx| InputState::new(window, cx).placeholder("game.exe or C:\\Games\\game.exe"));
        let subscription = cx.subscribe(&filter, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let poll = cx.spawn_in(window, async move |view, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(500)).await;
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            model,
            helper,
            filter,
            rule,
            rule_path: false,
            pending: None,
            notice: None,
            failed: false,
            _subscriptions: vec![subscription],
            _poll: poll,
        }
    }
    fn text(&self, cx: &App, zh: &'static str, en: &'static str) -> &'static str {
        if self.model.read(cx).settings.english {
            en
        } else {
            zh
        }
    }
    fn report(&mut self, result: anyhow::Result<()>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.notice = Some(self.text(cx, "已保存并应用", "Saved and applied").into());
                self.failed = false;
            }
            Err(error) => {
                self.notice = Some(format!("{error:#}").into());
                self.failed = true;
            }
        }
        cx.notify();
    }
    fn set_mode(&mut self, manual: bool, cx: &mut Context<Self>) {
        let mut next = self.model.read(cx).settings.clone();
        next.mode = if manual {
            SelectionMode::Manual
        } else {
            SelectionMode::Automatic
        };
        let result = self.model.update(cx, |model, _| model.apply(next));
        self.pending = None;
        self.report(result, cx);
    }
    fn lock(&mut self, target: ProcessIdentity, confirmed: bool, cx: &mut Context<Self>) {
        if !confirmed
            && self
                .model
                .read(cx)
                .settings
                .blacklist
                .iter()
                .any(|r| r.matches(&target))
        {
            self.pending = Some(target);
            cx.notify();
            return;
        }
        let result =
            if processes::identity(target.pid).is_none_or(|p| p.started_at != target.started_at) {
                Err(anyhow::anyhow!(
                    "Target exited or PID changed / 目标已退出或身份改变"
                ))
            } else {
                self.model.update(cx, |model, _| model.lock(target))
            };
        self.pending = None;
        self.report(result, cx);
    }
    fn add_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.rule.read(cx).value().to_string().trim().to_owned();
        let rule = if self.rule_path {
            BlacklistRule::FullPath(value)
        } else {
            BlacklistRule::FileName(value)
        };
        let mut next = self.model.read(cx).settings.clone();
        if !next.blacklist.contains(&rule) {
            next.blacklist.push(rule);
        }
        let result = self.model.update(cx, |model, _| model.apply(next));
        if result.is_ok() {
            self.rule
                .update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.report(result, cx);
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let settings = model.settings.clone();
        let snapshot = model.snapshot();
        let config_path: SharedString = model.store.path().display().to_string().into();
        let startup_error = model.startup_error.clone();
        let t = |zh, en| if settings.english { en } else { zh };
        let target: SharedString = snapshot.target.as_ref().map_or_else(
            || t("未选择目标", "No target selected").into(),
            |p| format!("{} · PID {} · Session {}", p.name, p.pid, snapshot.session).into(),
        );
        let status = t(
            match snapshot.phase {
                Phase::Waiting => "等待目标",
                Phase::Switching => "切换中",
                Phase::Capturing => "正在采集",
                Phase::Completed => "采集结束",
                Phase::Failed => "采集不可用",
            },
            match snapshot.phase {
                Phase::Waiting => "Waiting",
                Phase::Switching => "Switching",
                Phase::Capturing => "Capturing",
                Phase::Completed => "Capture ended",
                Phase::Failed => "Capture unavailable",
            },
        );
        let mut body=div().flex().flex_col().gap_5().p_6()
            .child(div().flex().justify_between().items_center()
                .child(div().flex().flex_col().gap_1().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(t("目标与设置","Targets and settings"))).child(div().text_sm().text_color(cx.theme().muted_foreground).child(t("非注入采集 · 不自动提权","Non-injected capture · no automatic elevation"))))
                .child(Button::new("quit-desktop").label(t("退出 NxMonitor","Quit NxMonitor")).on_click(|_,_,cx|cx.quit())))
            .children(startup_error.clone().map(|error| div().text_color(cx.theme().danger).child(error)))
            .children(self.notice.clone().map(|notice| div().text_sm().text_color(if self.failed {cx.theme().danger} else {cx.theme().success}).child(notice)))
            .child(div().flex().flex_col().gap_2().p_3().rounded_md().bg(cx.theme().group_box)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(target))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!("{status} · {} {}",snapshot.capture.received,t("帧","frames"))))
                .child(Button::new("restart-capture").small().label(t("重置并重新采集","Reset and restart capture")).on_click(cx.listener(|this,_,_,cx|{let result=this.model.read(cx).runtime.send(Command::Restart);this.report(result,cx);}))))
            .child(div().flex().flex_col().gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(t("选择方式","Selection mode")))
                .child(RadioGroup::new("selection-mode").children([t("跟随前台应用","Follow foreground application"),t("手动锁定","Manual lock")]).selected_index(Some(usize::from(settings.mode==SelectionMode::Manual))).on_change(cx.listener(|this,ix,_,cx|this.set_mode(*ix==1,cx))))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(t("自动模式在前台稳定约 650ms 后切换；打开本设置窗口不会切断当前目标。手动目标退出后不会跟随其他进程。","Auto follows a stable foreground after about 650ms. Opening these settings preserves the current target. Manual mode never follows another process after the target exits."))))
            .child(div().flex().flex_col().gap_2()
                .child(div().flex().justify_between().child(div().font_weight(FontWeight::SEMIBOLD).child(t("可见窗口进程","Windowed applications"))).child(Button::new("refresh-processes").small().label(t("刷新","Refresh")).on_click(cx.listener(|this,_,_,cx|{let result=this.model.read(cx).runtime.send(Command::Refresh);this.report(result,cx);}))))
                .child(Input::new(&self.filter)));
        if settings.mode == SelectionMode::Manual && snapshot.target.is_none() {
            let last: SharedString = settings
                .manual_path
                .clone()
                .unwrap_or_else(|| {
                    t(
                        "请在下方重新选择进程；不会恢复旧 PID。",
                        "Select a process below; old PIDs are never restored.",
                    )
                    .into()
                })
                .into();
            body = body.child(div().text_sm().text_color(cx.theme().warning).child(last));
        }
        if let Some(pending) = self.pending.clone() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().warning)
                    .rounded_md()
                    .child(div().child(format!(
                        "{} {}",
                        pending.name,
                        t(
                            "在黑名单中，仍要手动监测吗？",
                            "is blacklisted. Monitor it manually anyway?"
                        )
                    )))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("confirm-blacklist-override")
                                    .label(t("仍然锁定", "Lock anyway"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.lock(pending.clone(), true, cx)
                                    })),
                            )
                            .child(
                                Button::new("cancel-blacklist-override")
                                    .ghost()
                                    .label(t("取消", "Cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pending = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        let query = self.filter.read(cx).value().to_lowercase();
        let rows = snapshot
            .processes
            .iter()
            .filter(|p| {
                query.is_empty()
                    || p.identity.name.to_lowercase().contains(&query)
                    || p.identity.pid.to_string().contains(&query)
            })
            .collect::<Vec<_>>();
        let mut list = div().flex().flex_col().gap_2();
        if rows.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        "没有可识别的窗口进程。受保护进程可能不可读。",
                        "No accessible windowed processes. Protected processes may be unavailable.",
                    )),
            );
        }
        for process in rows.into_iter().take(150) {
            let identity = process.identity.clone();
            let blocked = settings.blacklist.iter().any(|r| r.matches(&identity));
            let title: SharedString = format!(
                "{} · PID {}{}",
                identity.name,
                identity.pid,
                if blocked {
                    t(" · 已排除自动选择", " · excluded from auto")
                } else {
                    ""
                }
            )
            .into();
            let path: SharedString = identity.path.clone().unwrap_or_default().into();
            let window_title: SharedString = process.title.clone().into();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().child(title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(window_title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(path),
                            ),
                    )
                    .child(
                        Button::new(("lock-process", identity.pid as u64))
                            .small()
                            .label(t("锁定", "Lock"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.lock(identity.clone(), false, cx)
                            })),
                    ),
            );
        }
        body = body.child(div().h_56().min_h_0().child(list.overflow_y_scrollbar()));
        let mut blacklist=div().flex().flex_col().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child(t("自动选择黑名单","Automatic selection blacklist")))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(t("仅影响自动选择。精确匹配，不区分大小写；完整路径不使用通配符。","Only affects automatic selection. Exact, case-insensitive matches; no wildcards.")))
            .child(RadioGroup::new("blacklist-rule-kind").children([t("文件名","File name"),t("完整路径","Full path")]).selected_index(Some(usize::from(self.rule_path))).on_change(cx.listener(|this,ix,_,cx|{this.rule_path=*ix==1;cx.notify();})))
            .child(div().flex().gap_2().child(div().flex_1().min_w_0().child(Input::new(&self.rule))).child(Button::new("add-blacklist-rule").label(t("添加","Add")).on_click(cx.listener(|this,_,window,cx|this.add_rule(window,cx)))));
        for (ix, rule) in settings.blacklist.iter().enumerate() {
            let label: SharedString = match rule {
                BlacklistRule::FileName(value) => format!("{}: {value}", t("文件名", "File name")),
                BlacklistRule::FullPath(value) => {
                    format!("{}: {value}", t("完整路径", "Full path"))
                }
            }
            .into();
            blacklist = blacklist.child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(div().flex_1().min_w_0().text_sm().child(label))
                    .child(
                        Button::new(("remove-blacklist-rule", ix))
                            .small()
                            .ghost()
                            .label(t("移除", "Remove"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let mut next = this.model.read(cx).settings.clone();
                                if ix < next.blacklist.len() {
                                    next.blacklist.remove(ix);
                                }
                                let result = this.model.update(cx, |model, _| model.apply(next));
                                this.report(result, cx);
                            })),
                    ),
            );
        }
        body = body
            .child(blacklist)
            .child(device_panel(&snapshot.device, settings.english, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t("显示与采集器", "Display and collector")),
                    )
                    .child(
                        Switch::new("english-language")
                            .label("English interface")
                            .checked(settings.english)
                            .on_change(cx.listener(|this, value, _, cx| {
                                let mut next = this.model.read(cx).settings.clone();
                                next.english = *value;
                                let result = this.model.update(cx, |model, _| model.apply(next));
                                this.report(result, cx);
                            })),
                    )
                    .child(
                        Switch::new("graphy-style")
                            .label(t("Graphy 紧凑预设", "Graphy compact preset"))
                            .checked(settings.graphy)
                            .on_change(cx.listener(|this, value, _, cx| {
                                let mut next = this.model.read(cx).settings.clone();
                                next.graphy = *value;
                                let result = this.model.update(cx, |model, _| model.apply(next));
                                this.report(result, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(t("PresentMon 可执行文件路径", "PresentMon executable path")),
                    )
                    .child(Input::new(&self.helper))
                    .child(
                        Button::new("save-helper-path")
                            .label(t("保存采集器路径", "Save collector path"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let mut next = this.model.read(cx).settings.clone();
                                next.presentmon_path = this.helper.read(cx).value().to_string();
                                let result = this.model.update(cx, |model, _| model.apply(next));
                                this.report(result, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(config_path),
            );
        let diagnostic = snapshot
            .capture
            .error
            .as_deref()
            .unwrap_or(&snapshot.message);
        if !diagnostic.is_empty() {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(diagnostic.to_owned())),
            );
        }
        div()
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(body.overflow_y_scrollbar())
    }
}

fn device_panel(snapshot: &super::device::Snapshot, english: bool, cx: &App) -> impl IntoElement {
    let t = |zh, en| if english { en } else { zh };
    let inventory = &snapshot.inventory;
    let unknown = t("不可用", "Unavailable");
    let cpu: SharedString = inventory
        .cpu_name
        .clone()
        .unwrap_or_else(|| unknown.into())
        .into();
    let count = |n: Option<u32>| n.map_or_else(|| "—".into(), |n| n.to_string());
    let os: SharedString = inventory
        .os
        .clone()
        .unwrap_or_else(|| unknown.into())
        .into();
    let display: SharedString = if !snapshot.fresh() {
        "—".into()
    } else {
        match &snapshot.display {
            Ok(display) => display.summary(english).into(),
            Err(error) => error.label(english).into(),
        }
    };
    let mut panel = div()
        .flex()
        .flex_col()
        .gap_2()
        .border_t_1()
        .border_color(cx.theme().border)
        .pt_4()
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(t("设备与显示信息", "Device and display information")),
        )
        .child(div().text_sm().child(cpu))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "{} {} · {} {}",
                    count(inventory.physical_cores),
                    t("物理核心", "physical cores"),
                    count(inventory.logical_processors),
                    t("逻辑处理器", "logical processors")
                )),
        )
        .child(div().text_sm().child(os))
        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(t(
            "本机 GPU 适配器（非目标使用判定）",
            "Local GPU adapters (not target GPU selection)",
        )));
    if inventory.gpus.is_empty() {
        panel = panel.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if inventory.gpu_error {
                    t("GPU 查询不可用", "GPU query unavailable")
                } else {
                    t("未枚举到硬件适配器", "No hardware adapters enumerated")
                }),
        );
    }
    for (ix, gpu) in inventory.gpus.iter().enumerate() {
        panel = panel.child(
            div()
                .text_sm()
                .child(SharedString::from(format!("GPU {}: {gpu}", ix + 1))),
        );
    }
    panel.child(div().text_sm().child(display))
        .child(div().text_sm().child(format!("{}: {}", t("目标客户区（物理像素）", "Target client area (physical pixels)"), snapshot.window_summary(english))))
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(t("设备清单启动时缓存；窗口/屏幕每秒刷新。刷新率为系统报告的整数Hz，不是目标FPS。未检测目标图形API。", "Inventory cached at startup; window/display refreshed every second. Refresh rate is system-reported integer Hz, not target FPS. Target graphics API is not detected.")))
}
