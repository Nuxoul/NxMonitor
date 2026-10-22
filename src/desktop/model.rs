use super::runtime::{Command, Runtime, RuntimeSnapshot};
use anyhow::Result;
use gpui_kit::*;
use nxmonitor::{
    config::{ConfigStore, SelectionMode, Settings},
    target::ProcessIdentity,
};

pub struct DesktopModel {
    pub settings: Settings,
    pub store: ConfigStore,
    pub runtime: Runtime,
    pub settings_window: Option<AnyWindowHandle>,
    pub startup_error: Option<SharedString>,
}
impl DesktopModel {
    pub fn new(
        store: ConfigStore,
        settings: Settings,
        error: Option<String>,
        seconds: u32,
    ) -> Self {
        Self {
            runtime: Runtime::new(settings.clone(), seconds),
            settings,
            store,
            settings_window: None,
            startup_error: error.map(Into::into),
        }
    }
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.runtime.snapshot()
    }
    pub fn apply(&mut self, next: Settings) -> Result<()> {
        anyhow::ensure!(
            self.startup_error.is_none(),
            "Existing configuration could not be loaded. Fix or rename that file before saving; it has not been overwritten."
        );
        next.validate()?;
        self.runtime
            .commit(Command::Apply(next.clone()), || self.store.save(&next))?;
        self.settings = next;
        Ok(())
    }
    pub fn lock(&mut self, target: ProcessIdentity) -> Result<()> {
        anyhow::ensure!(
            self.startup_error.is_none(),
            "Fix the existing configuration before saving a target."
        );
        let mut next = self.settings.clone();
        next.mode = SelectionMode::Manual;
        next.manual_path = target.path.clone();
        next.validate()?;
        self.runtime
            .commit(Command::Lock(next.clone(), target), || {
                self.store.save(&next)
            })?;
        self.settings = next;
        Ok(())
    }
}

pub fn open_settings(model: &Entity<DesktopModel>, cx: &mut App) {
    if let Some(handle) = model.read(cx).settings_window
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let target = model.clone();
    let result = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(470.), px(80.)),
                size(px(820.), px(820.)),
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("NxMonitor — Settings / 设置".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(660.), px(560.))),
            ..Default::default()
        },
        cx,
        move |window, cx| cx.new(|cx| super::settings::SettingsView::new(target, window, cx)),
    );
    match result {
        Ok((handle, _)) => model.update(cx, |model, _| model.settings_window = Some(handle)),
        Err(error) => eprintln!("SETTINGS_WINDOW_ERROR {error:#}"),
    }
}
