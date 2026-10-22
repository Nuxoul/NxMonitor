use super::device;
use super::memory;
use super::processes::{self, WindowProcess};
use nxmonitor::{
    capture::{Capture, CaptureState},
    config::{SelectionMode, Settings},
    target::{ProcessIdentity, Selection},
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Waiting,
    Switching,
    Capturing,
    Completed,
    Failed,
}
#[derive(Clone)]
pub struct RuntimeSnapshot {
    pub processes: Vec<WindowProcess>,
    pub target: Option<ProcessIdentity>,
    pub capture: CaptureState,
    pub phase: Phase,
    pub message: String,
    pub session: u64,
    pub memory: memory::Snapshot,
    pub device: device::Snapshot,
}
pub enum Command {
    Apply(Settings),
    Lock(Settings, ProcessIdentity),
    Restart,
    Refresh,
}

pub struct Runtime {
    pub shared: Arc<Mutex<RuntimeSnapshot>>,
    queue: Arc<Mutex<VecDeque<Command>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Runtime {
    pub fn new(settings: Settings, capture_seconds: u32) -> Self {
        let shared = Arc::new(Mutex::new(RuntimeSnapshot {
            processes: Vec::new(),
            target: None,
            capture: CaptureState::default(),
            phase: Phase::Waiting,
            message: String::new(),
            session: 0,
            memory: memory::Snapshot::default(),
            device: device::Snapshot::default(),
        }));
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let worker_queue = queue.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let state = shared.clone();
        let stopping = stop.clone();
        let worker =
            thread::spawn(move || run(settings, capture_seconds, worker_queue, state, stopping));
        Self {
            shared,
            queue,
            stop,
            worker: Some(worker),
        }
    }
    pub fn send(&self, command: Command) -> anyhow::Result<()> {
        self.commit(command, || Ok(()))
    }
    pub fn commit(
        &self,
        command: Command,
        persist: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let mut queue = self.queue.lock().unwrap();
        anyhow::ensure!(
            !self.stop.load(Ordering::Relaxed)
                && self.worker.as_ref().is_some_and(|w| !w.is_finished()),
            "target controller unavailable"
        );
        anyhow::ensure!(
            queue.len() < 16,
            "target controller busy; settings were not saved"
        );
        // Hold the reservation through atomic file replacement. Worker sees the command only after success.
        persist()?;
        queue.push_back(command);
        Ok(())
    }
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.shared.lock().unwrap().clone()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn same(a: &ProcessIdentity, b: &ProcessIdentity) -> bool {
    a.pid == b.pid && a.started_at == b.started_at
}
fn same_target(a: &Option<ProcessIdentity>, b: &Option<ProcessIdentity>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (None, None) => true,
        _ => false,
    }
}
fn helper_path(value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    // Resolve the default tools path relative to the project/installed application, not an arbitrary caller CWD.
    let executable = std::env::current_exe().unwrap_or_default();
    let dir = executable.parent().unwrap_or(std::path::Path::new("."));
    if dir
        .file_name()
        .is_some_and(|n| n == "debug" || n == "release")
        && dir
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n == "target")
    {
        dir.parent()
            .and_then(|p| p.parent())
            .unwrap_or(dir)
            .join(path)
    } else {
        dir.join(path)
    }
}
fn run(
    mut settings: Settings,
    seconds: u32,
    queue: Arc<Mutex<VecDeque<Command>>>,
    shared: Arc<Mutex<RuntimeSnapshot>>,
    stop: Arc<AtomicBool>,
) {
    let own = std::process::id();
    let mut selection = Selection::new(settings.mode);
    let mut locked: Option<ProcessIdentity> = None;
    let mut active: Option<ProcessIdentity> = None;
    let mut pending: Option<ProcessIdentity> = None;
    let mut pending_since = Instant::now();
    let mut process_refresh = Instant::now() - Duration::from_secs(2);
    let mut processes = Vec::<WindowProcess>::new();
    let mut capture: Option<Capture> = None;
    let mut state = CaptureState::default();
    let mut phase = Phase::Waiting;
    let mut message = String::new();
    let mut session = 0;
    let mut device = device::Snapshot::new();
    let mut device_refresh = Instant::now() - device::INTERVAL;
    let mut memory = memory::Snapshot::default();
    let mut memory_refresh = Instant::now() - memory::SAMPLE_INTERVAL;
    while !stop.load(Ordering::Relaxed) {
        let mut restart = false;
        let commands: Vec<_> = queue.lock().unwrap().drain(..).collect();
        for command in commands {
            match command {
                Command::Apply(next) => {
                    restart |= settings.presentmon_path != next.presentmon_path;
                    settings = next;
                    selection.set_mode(settings.mode);
                    if settings.mode == SelectionMode::Automatic {
                        locked = None;
                    }
                    if settings.mode == SelectionMode::Manual
                        && let Some(target) = locked.clone()
                    {
                        selection.lock(target);
                    }
                }
                Command::Lock(next, target) => {
                    settings = next;
                    locked = Some(target.clone());
                    selection.lock(target);
                }
                Command::Restart => restart = true,
                Command::Refresh => process_refresh = Instant::now() - Duration::from_secs(2),
            }
        }
        if process_refresh.elapsed() >= Duration::from_millis(750) {
            processes = processes::window_processes(own);
            process_refresh = Instant::now();
        }
        // A locked minimized app can lose a visible window; liveness is checked via process identity.
        let mut running: Vec<_> = processes.iter().map(|p| p.identity.clone()).collect();
        if let Some(target) = &locked
            && !running.iter().any(|p| same(p, target))
            && let Some(alive) = processes::identity(target.pid)
        {
            running.push(alive);
        }
        let foreground_pid = processes::foreground_pid();
        let foreground = running.iter().find(|p| p.pid == foreground_pid);
        let desired = if settings.mode == SelectionMode::Automatic && foreground_pid == own {
            // Opening NxMonitor settings must not interrupt the game being inspected.
            active
                .as_ref()
                .and_then(|p| processes::identity(p.pid).filter(|alive| same(p, alive)))
                .filter(|p| !settings.blacklist.iter().any(|r| r.matches(p)))
        } else {
            selection.resolve(foreground, &running, &settings.blacklist, own)
        };
        let desired =
            if settings.mode == SelectionMode::Automatic && !same_target(&desired, &active) {
                if !same_target(&desired, &pending) {
                    pending = desired.clone();
                    pending_since = Instant::now();
                }
                if pending_since.elapsed() >= Duration::from_millis(650) {
                    desired
                } else {
                    active.clone()
                }
            } else {
                pending = desired.clone();
                desired
            };
        let changed = !same_target(&desired, &active);
        if changed || restart {
            {
                let mut out = shared.lock().unwrap();
                out.phase = Phase::Switching;
                out.capture = CaptureState::default();
                memory.select(desired.as_ref());
                out.memory = memory.clone();
                device.select(desired.as_ref());
                out.device = device.clone();
                out.message = "Stopping previous capture / 正在切换采集".into();
            }
            capture.take(); // Bounded owned ETW cleanup happens on this worker, never in render/UI callbacks.
            state = CaptureState::default();
            active = desired;
            message.clear();
            if let Some(target) = &active {
                if processes::identity(target.pid).is_none_or(|alive| !same(&alive, target)) {
                    active = None;
                    phase = Phase::Waiting;
                    message = "Target exited before capture / 目标已退出".into();
                } else {
                    session += 1;
                    eprintln!(
                        "TARGET_SESSION session={session} pid={} mode={:?}",
                        target.pid, settings.mode
                    );
                    match Capture::start(
                        &helper_path(&settings.presentmon_path),
                        target.pid,
                        seconds,
                    ) {
                        Ok(started) => {
                            capture = Some(started);
                            phase = Phase::Capturing;
                        }
                        Err(error) => {
                            phase = Phase::Failed;
                            message = format!("{error:#}");
                        }
                    }
                }
            } else {
                phase = Phase::Waiting;
            }
        }
        if let Some(target) = &active
            && processes::identity(target.pid).is_none_or(|alive| !same(&alive, target))
        {
            capture.take();
            active = None;
            state = CaptureState::default();
            phase = Phase::Waiting;
            message = "Target exited / 目标已退出".into();
        }
        if let Some(collector) = &mut capture {
            match collector.poll() {
                Ok(next) => {
                    state = next;
                    phase = if state.error.is_some() {
                        Phase::Failed
                    } else if state.finished {
                        Phase::Completed
                    } else {
                        Phase::Capturing
                    };
                }
                Err(error) => {
                    phase = Phase::Failed;
                    message = format!("{error:#}");
                }
            }
        }
        let memory_target_changed = memory.target_id != active.as_ref().map(ProcessIdentity::id);
        memory.select(active.as_ref());
        if memory_target_changed || memory_refresh.elapsed() >= memory::SAMPLE_INTERVAL {
            memory.sample(active.as_ref());
            memory_refresh = Instant::now();
        }
        let device_target_changed = device.target_id != active.as_ref().map(ProcessIdentity::id);
        device.select(active.as_ref());
        if device_target_changed
            || device.sampled_at.is_none()
            || device_refresh.elapsed() >= device::INTERVAL
        {
            device.sample(active.as_ref());
            device_refresh = Instant::now();
        }
        {
            let mut out = shared.lock().unwrap();
            *out = RuntimeSnapshot {
                processes: processes.clone(),
                target: active.clone(),
                capture: state.clone(),
                phase,
                message: message.clone(),
                session,
                memory: memory.clone(),
                device: device.clone(),
            };
        }
        thread::sleep(Duration::from_millis(100));
    }
    capture.take();
    eprintln!(
        "DESKTOP_CONTROLLER_STOPPED session={session} received={}",
        state.received
    );
}

#[cfg(test)]
mod commit_tests {
    use super::*;
    #[test]
    fn config_commit_reserves_capacity_before_persist_and_never_queues_failed_save() {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
            }
        });
        let runtime = Runtime {
            shared: Arc::new(Mutex::new(RuntimeSnapshot {
                processes: Vec::new(),
                target: None,
                capture: CaptureState::default(),
                phase: Phase::Waiting,
                message: String::new(),
                session: 0,
                memory: memory::Snapshot::default(),
                device: device::Snapshot::default(),
            })),
            queue: Arc::new(Mutex::new((0..16).map(|_| Command::Refresh).collect())),
            stop,
            worker: Some(worker),
        };
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        assert!(
            runtime
                .commit(Command::Restart, || {
                    std::fs::write(&file, "new")?;
                    Ok(())
                })
                .is_err()
        );
        assert!(!file.exists());
        runtime.queue.lock().unwrap().clear();
        assert!(
            runtime
                .commit(Command::Restart, || anyhow::bail!("disk unavailable"))
                .is_err()
        );
        assert!(runtime.queue.lock().unwrap().is_empty());
        runtime
            .commit(Command::Restart, || {
                std::fs::write(&file, "saved")?;
                Ok(())
            })
            .unwrap();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "saved");
        assert_eq!(runtime.queue.lock().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod memory_integration_tests {
    use super::*;
    fn wait_for(runtime: &Runtime, check: impl Fn(&RuntimeSnapshot) -> bool) -> RuntimeSnapshot {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let snapshot = runtime.snapshot();
            if check(&snapshot) {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "memory controller did not reach expected state"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn child() -> Child {
        Child(
            std::process::Command::new("cmd.exe")
                .args(["/C", "pause"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    #[test]
    fn memory_updates_without_fps_and_resets_on_switch_and_exit() {
        let a = child();
        let mut b = child();
        let first = processes::identity(a.0.id()).unwrap();
        let second = processes::identity(b.0.id()).unwrap();
        let mut settings = Settings::default();
        settings.mode = SelectionMode::Manual;
        let dir = tempfile::tempdir().unwrap();
        settings.presentmon_path = dir.path().join("missing-helper.exe").display().to_string();
        let runtime = Runtime::new(settings.clone(), 1);
        wait_for(&runtime, |s| s.memory.system.is_ok());
        runtime
            .send(Command::Lock(settings.clone(), first.clone()))
            .unwrap();
        wait_for(&runtime, |s| {
            s.memory.target_id == Some(first.id()) && s.memory.working_set_bytes.is_ok()
        });
        runtime
            .send(Command::Lock(settings, second.clone()))
            .unwrap();
        let switched = wait_for(&runtime, |s| {
            s.memory.target_id == Some(second.id()) && s.memory.working_set_bytes.is_ok()
        });
        assert!(switched.memory.process_history.len() <= 1);
        assert_eq!(switched.phase, Phase::Failed); // Memory does not depend on the FPS helper.
        b.0.kill().unwrap();
        b.0.wait().unwrap();
        let exited = wait_for(&runtime, |s| {
            s.target.is_none() && s.memory.target_id.is_none()
        });
        assert!(exited.memory.process_history.is_empty());
        assert_eq!(
            exited.memory.working_set_bytes,
            Err(memory::Unavailable::NoTarget)
        );
    }
}
