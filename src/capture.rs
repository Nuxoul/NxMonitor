use crate::{
    presentmon::Schema,
    stats::{Snapshot, Statistics},
};
use anyhow::{Context, Result, ensure};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct CaptureState {
    pub snapshot: Snapshot,
    pub received: usize,
    pub rejected: usize,
    pub last_frame: Option<Instant>,
    pub finished: bool,
    pub error: Option<String>,
    pub diagnostics: String,
}

impl CaptureState {
    fn finish(&mut self, success: bool, exit: &str) {
        self.finished = true;
        if self.error.is_some() {
            return;
        }
        if !success {
            self.error = Some(format!(
                "PresentMon exited {exit}. Check capture permissions; NxMonitor does not elevate automatically."
            ));
        } else if self.received == 0 {
            self.error = Some("No target frames received. Check the PID, target rendering activity and capture permissions.".into());
        }
    }
}
/// Owns one uniquely named, bounded ETW helper session and its pipe readers.
pub struct Capture {
    child: Child,
    executable: PathBuf,
    session: String,
    state: Arc<Mutex<CaptureState>>,
    reader: Option<JoinHandle<()>>,
    diagnostics: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(executable: &Path, pid: u32, seconds: u32) -> Result<Self> {
        ensure!(
            pid != 0 && pid != std::process::id(),
            "select a nonzero external PID, not NxMonitor itself"
        );
        ensure!(
            (1..=600).contains(&seconds),
            "validation duration must be 1..600 seconds"
        );
        ensure!(
            executable.is_file(),
            "PresentMon executable not found: {}",
            executable.display()
        );
        let session = format!(
            "NxMonitor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        );
        let mut command = Command::new(executable);
        command.args([
            "--process_id",
            &pid.to_string(),
            "--output_stdout",
            "--v1_metrics",
            "--timed",
            &seconds.to_string(),
            "--terminate_after_timed",
            "--session_name",
            &session,
        ]);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        hide_console(&mut command);
        let mut child = command
            .spawn()
            .context("failed to start PresentMon; no automatic elevation was requested")?;
        let stdout = child.stdout.take().context("missing capture stdout")?;
        let stderr = child.stderr.take().context("missing capture stderr")?;
        let state = Arc::new(Mutex::new(CaptureState::default()));
        let shared = state.clone();
        let reader = thread::spawn(move || {
            let result = (|| -> Result<()> {
                let mut csv = csv::ReaderBuilder::new().from_reader(stdout);
                let header = csv.headers()?.clone();
                // PresentMon emits no header at all when the target never presents.
                if header.is_empty() {
                    return Ok(());
                }
                let schema = Schema::new(&header)?;
                let mut stats = Statistics::new(30.0);
                let mut refresh = Instant::now();
                for record in csv.records() {
                    let row = record?;
                    match schema.parse(&row)? {
                        Some(frame) if frame.pid == pid => {
                            if stats.push(frame) {
                                let mut state = shared.lock().unwrap();
                                state.received += 1;
                                state.last_frame = Some(Instant::now());
                                if refresh.elapsed() >= Duration::from_millis(250)
                                    || state.received == 1
                                {
                                    state.snapshot = stats.snapshot();
                                    refresh = Instant::now();
                                }
                            } else {
                                shared.lock().unwrap().rejected += 1;
                            }
                        }
                        _ => {
                            shared.lock().unwrap().rejected += 1;
                        }
                    }
                }
                shared.lock().unwrap().snapshot = stats.snapshot();
                Ok(())
            })();
            if let Err(error) = result {
                shared.lock().unwrap().error = Some(format!("{error:#}"));
            }
        });
        let shared = state.clone();
        let diagnostics = thread::spawn(move || {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let mut state = shared.lock().unwrap();
                if state.diagnostics.len() < 16_384 {
                    state.diagnostics.push_str(&line);
                    state.diagnostics.push('\n');
                }
            }
        });
        Ok(Self {
            child,
            executable: executable.to_owned(),
            session,
            state,
            reader: Some(reader),
            diagnostics: Some(diagnostics),
        })
    }

    pub fn poll(&mut self) -> Result<CaptureState> {
        if let Some(exit) = self.child.try_wait()? {
            self.join_readers();
            let mut state = self.state.lock().unwrap();
            state.finish(exit.success(), &exit.to_string());
        }
        Ok(self.state.lock().unwrap().clone())
    }

    fn join_readers(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.diagnostics.take() {
            let _ = reader.join();
        }
    }
}

impl Capture {
    fn stop_owned_session(&self) {
        let mut stop = Command::new(&self.executable);
        stop.args([
            "--session_name",
            &self.session,
            "--terminate_existing_session",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        hide_console(&mut stop);
        if let Ok(mut helper) = stop.spawn() {
            for _ in 0..100 {
                if helper.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let _ = helper.kill();
            let _ = helper.wait();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Empty/early-ended streams may leave a trace even if the helper already exited.
        // Stop only this instance's unique name; never touch the default PresentMon session.
        self.stop_owned_session();
        if self.child.try_wait().ok().flatten().is_none() {
            for _ in 0..60 {
                if self.child.try_wait().ok().flatten().is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
                let _ = self.child.wait();
                self.stop_owned_session();
            }
        }
        self.join_readers();
    }
}

fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW, no UAC prompt.
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_global_or_self_capture_and_unbounded_duration() {
        assert!(Capture::start(Path::new("missing.exe"), 0, 10).is_err());
        assert!(Capture::start(Path::new("missing.exe"), std::process::id(), 10).is_err());
        assert!(Capture::start(Path::new("missing.exe"), 42, 601).is_err());
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    #[test]
    fn empty_success_is_unavailable_and_existing_errors_are_preserved() {
        let mut state = CaptureState::default();
        state.finish(true, "0");
        assert!(state.finished);
        assert!(state.error.as_deref().unwrap().contains("No target frames"));
        state.error = Some("original schema error".into());
        state.finish(false, "1");
        assert_eq!(state.error.as_deref(), Some("original schema error"));
        let mut valid = CaptureState {
            received: 1,
            ..Default::default()
        };
        valid.finish(true, "0");
        assert!(valid.error.is_none());
    }
}
