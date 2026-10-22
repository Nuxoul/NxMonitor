//! Read-only Windows process discovery, confined to this platform boundary.
use nxmonitor::target::ProcessIdentity;
use std::collections::BTreeMap;
use windows::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, RECT},
        System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowRect,
            GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        },
    },
    core::{BOOL, PWSTR},
};

#[derive(Clone, Debug)]
pub struct WindowProcess {
    pub identity: ProcessIdentity,
    pub title: String,
}

struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn identity(pid: u32) -> Option<ProcessIdentity> {
    if pid == 0 || pid == 4 {
        return None;
    }
    // SAFETY: query-only owned handle, closed by RAII; all buffers are live and sized.
    unsafe {
        let process =
            ProcessHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user).ok()?;
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        let name = path.rsplit(['\\', '/']).next()?.to_owned();
        let started_at = (created.dwHighDateTime as u64) << 32 | created.dwLowDateTime as u64;
        Some(ProcessIdentity::new(pid, started_at, name, Some(path)))
    }
}

pub fn foreground_pid() -> u32 {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid
    }
}

pub fn window_processes(own_pid: u32) -> Vec<WindowProcess> {
    let mut windows: Vec<(u32, String)> = Vec::new();
    // SAFETY: EnumWindows invokes synchronously and the Vec pointer lives for the call.
    unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
        unsafe {
            if !IsWindowVisible(hwnd).as_bool()
                || GetWindow(hwnd, GW_OWNER)
                    .ok()
                    .is_some_and(|h| !h.is_invalid())
            {
                return true.into();
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err()
                || rect.right - rect.left < 80
                || rect.bottom - rect.top < 60
            {
                return true.into();
            }
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return true.into();
            }
            let mut title = vec![0u16; len as usize + 1];
            let written = GetWindowTextW(hwnd, &mut title);
            if written <= 0 {
                return true.into();
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let windows = &mut *(data.0 as *mut Vec<(u32, String)>);
            windows.push((pid, String::from_utf16_lossy(&title[..written as usize])));
            true.into()
        }
    }
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize));
    }
    let mut unique = BTreeMap::new();
    for (pid, title) in windows {
        if pid == own_pid || unique.contains_key(&pid) {
            continue;
        }
        if let Some(identity) = identity(pid) {
            unique.insert(pid, WindowProcess { identity, title });
        }
    }
    let mut result: Vec<_> = unique.into_values().collect();
    result.sort_by(|a, b| {
        a.identity
            .name
            .to_lowercase()
            .cmp(&b.identity.name.to_lowercase())
            .then(a.identity.pid.cmp(&b.identity.pid))
    });
    result
}
