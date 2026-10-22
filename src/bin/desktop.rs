use anyhow::{Context as _, Result, ensure};
use gpui_kit::{
    component::{ActiveTheme, Theme, ThemeMode},
    *,
};
use nxmonitor::config::{ConfigStore, Settings};
use std::{path::PathBuf, time::Duration};
#[path = "../desktop/device.rs"]
mod device;
#[path = "../desktop/hud.rs"]
mod hud;
#[path = "../desktop/memory.rs"]
mod memory;
#[path = "../desktop/model.rs"]
mod model;
#[path = "../desktop/processes.rs"]
mod processes;
#[path = "../desktop/runtime.rs"]
mod runtime;
#[path = "../desktop/settings.rs"]
mod settings;

fn main() -> Result<()> {
    let mut config = None;
    let mut smoke = None;
    let mut seconds = 600;
    let mut list = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config = Some(PathBuf::from(args.next().context("missing config path")?)),
            "--smoke-seconds" => {
                smoke = Some(
                    args.next()
                        .context("missing smoke seconds")?
                        .parse::<u64>()?,
                )
            }
            "--capture-seconds" => {
                seconds = args
                    .next()
                    .context("missing capture duration")?
                    .parse::<u32>()?
            }
            "--list-processes" => list = true,
            "--help" => {
                println!(
                    "NxMonitor desktop / 桌面入口\n--config <path>  Override %LOCALAPPDATA%\\NxMonitor\\settings.json\n--list-processes  Print accessible windowed processes\n--smoke-seconds <n>  Auto-close smoke test\n--capture-seconds <1..600>  Bounded session (default 600); restart in settings\n"
                );
                return Ok(());
            }
            _ => anyhow::bail!("unknown option: {arg}"),
        }
    }
    ensure!(
        (1..=600).contains(&seconds),
        "capture seconds must be 1..600"
    );
    if list {
        for process in processes::window_processes(std::process::id()) {
            println!(
                "{}\t{}\t{}",
                process.identity.pid, process.identity.name, process.title
            );
        }
        return Ok(());
    }
    let store = match config {
        Some(path) => ConfigStore::new(if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        }),
        None => ConfigStore::app_default()?,
    };
    let (settings, error) = match store.load() {
        Ok(settings) => (settings, None),
        Err(error) => (
            {
                let mut safe = Settings::default();
                safe.mode = nxmonitor::config::SelectionMode::Manual;
                safe
            },
            Some(format!(
                "Configuration not loaded; original file preserved / 配置未加载，原文件已保留：{error:#}"
            )),
        ),
    };
    eprintln!("DESKTOP_CONFIG {}", store.path().display());
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        let model = cx.new(|_| model::DesktopModel::new(store, settings, error, seconds));
        let target = model.clone();
        let (handle, _) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(40.), px(60.)),
                    size(px(380.), px(850.)),
                ))),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| hud::DesktopHud::new(target, window, cx)),
        )
        .expect("open NxMonitor HUD");
        handle
            .update(cx, |_, window, cx| {
                gpui_kit::base::Root::update(window, cx, |root, _, cx| {
                    root.style().background = Some(cx.theme().background.opacity(0.).into());
                    cx.notify();
                })
            })
            .expect("set HUD surface");
        model::open_settings(&model, cx);
        // This one-shot owns application lifetime, not view state; detached intentionally.
        if let Some(seconds) = smoke {
            cx.spawn(async move |cx| {
                smol::Timer::after(Duration::from_secs(seconds)).await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
    Ok(())
}
