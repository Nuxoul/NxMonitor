//! Windows-only capability boundary. HWND stays on the owning GPUI thread.
use anyhow::{Context, Result, bail};
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
        },
        WindowsAndMessaging::*,
    },
};

const EDIT_ID: i32 = 0x4e01;
const QUIT_ID: i32 = 0x4e02;
pub enum Command {
    Edit,
    Quit,
}

pub struct OverlayWindow {
    hwnd: HWND,
    commands: Receiver<Command>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    pub edit_shortcut: &'static str,
    pub quit_shortcut: &'static str,
}
impl OverlayWindow {
    pub fn new(window: &Window) -> Result<Self> {
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|e| anyhow::anyhow!("no native window handle: {e}"))?;
        let RawWindowHandle::Win32(raw) = handle.as_raw() else {
            bail!("Windows x64 is required")
        };
        let (commands_tx, commands) = mpsc::sync_channel(8);
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        // Thread hotkeys need their own pump; GPUI may consume WM_HOTKEY before a UI timer sees it.
        let worker = thread::spawn(move || {
            let register = (|| -> Result<(&'static str, &'static str)> {
                for (edit, quit, edit_label, quit_label) in [
                    (0x75, 0x76, "Ctrl+Alt+F6", "Ctrl+Alt+F7"),
                    (0x77, 0x78, "Ctrl+Alt+F8", "Ctrl+Alt+F9"),
                    (0x79, 0x7a, "Ctrl+Alt+F10", "Ctrl+Alt+F11"),
                ] {
                    unsafe {
                        if RegisterHotKey(None, EDIT_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, edit)
                            .is_err()
                        {
                            continue;
                        }
                        if RegisterHotKey(None, QUIT_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, quit)
                            .is_err()
                        {
                            let _ = UnregisterHotKey(None, EDIT_ID);
                            continue;
                        }
                    }
                    return Ok((edit_label, quit_label));
                }
                bail!("no available global edit/quit shortcut pair")
            })();
            let ok = register.is_ok();
            let _ = ready_tx.send(register.map_err(|e| format!("{e:#}")));
            if !ok {
                return;
            }
            while !stopping.load(Ordering::Relaxed) {
                unsafe {
                    let mut message = MSG::default();
                    while PeekMessageW(&mut message, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE)
                        .as_bool()
                    {
                        let command = match message.wParam.0 as i32 {
                            EDIT_ID => Some(Command::Edit),
                            QUIT_ID => Some(Command::Quit),
                            _ => None,
                        };
                        if let Some(command) = command {
                            let _ = commands_tx.try_send(command);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(20));
            }
            unsafe {
                let _ = UnregisterHotKey(None, EDIT_ID);
                let _ = UnregisterHotKey(None, QUIT_ID);
            }
        });
        let mut this = Self {
            hwnd: HWND(raw.hwnd.get() as *mut _),
            commands,
            stop,
            worker: Some(worker),
            edit_shortcut: "",
            quit_shortcut: "",
        };
        let registration = ready
            .recv_timeout(Duration::from_secs(3))
            .context("hotkey registration timeout")?;
        let (edit, quit) = registration.map_err(|error| {
            anyhow::anyhow!("{error}; refusing pass-through without escape keys")
        })?;
        this.edit_shortcut = edit;
        this.quit_shortcut = quit;
        eprintln!("HOTKEYS edit={edit} quit={quit}");
        this.set_editing(false)?;
        Ok(this)
    }
    pub fn set_editing(&mut self, editing: bool) -> Result<()> {
        // SAFETY: HWND belongs to this live GPUI window on this UI thread. GPUI retains its WndProc.
        // Layered + transparent is required for cross-process click-through; verified on Win10.
        unsafe {
            let mut style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            style |= WS_EX_TOOLWINDOW.0 | WS_EX_LAYERED.0;
            if editing {
                style &= !(WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0);
            } else {
                style |= WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0;
            }
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, style as isize);
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )?;
            let applied = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            anyhow::ensure!(
                (applied & WS_EX_TRANSPARENT.0 != 0) != editing,
                "pass-through style did not apply"
            );
            eprintln!(
                "NATIVE editing={editing} topmost={} layered={} passthrough={} hwnd={}",
                applied & WS_EX_TOPMOST.0 != 0,
                applied & WS_EX_LAYERED.0 != 0,
                applied & WS_EX_TRANSPARENT.0 != 0,
                self.hwnd.0 as usize
            );
        }
        Ok(())
    }
    pub fn drain_commands(&self) -> Vec<Command> {
        self.commands.try_iter().collect()
    }
    pub fn start_move(&self) {
        // Only called on the explicit editing header; Windows owns drag geometry.
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd),
                WM_NCLBUTTONDOWN,
                WPARAM(HTCAPTION as usize),
                LPARAM(0),
            );
        }
    }
}
impl Drop for OverlayWindow {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
