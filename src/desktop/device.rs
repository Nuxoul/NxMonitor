//! Read-only device inventory and physical-pixel window/display queries.
//! Inventory is cached by runtime; dynamic sampling never runs in render.
use super::processes;
use nxmonitor::target::ProcessIdentity;
use std::{
    mem::size_of,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT},
        Graphics::{
            Dxgi::{
                CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
            },
            Gdi::{
                DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW, HMONITOR,
                MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY, MONITORINFOEXW, MonitorFromPoint,
                MonitorFromWindow,
            },
        },
        System::{
            Registry::{HKEY, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW},
            SystemInformation::{GetLogicalProcessorInformationEx, RelationProcessorCore},
            Threading::{ALL_PROCESSOR_GROUPS, GetActiveProcessorCount},
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                SetThreadDpiAwarenessContext,
            },
            WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GetClientRect, GetForegroundWindow, GetWindow,
                GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            },
        },
    },
    core::{BOOL, PCWSTR, w},
};

pub const INTERVAL: Duration = Duration::from_secs(1);
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    pub cpu_name: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_processors: Option<u32>,
    pub os: Option<String>,
    /// All enumerated hardware adapters, not a claim about the target's active adapter.
    pub gpus: Vec<String>,
    pub gpu_error: bool,
}
impl Inventory {
    pub fn read() -> Self {
        let cpu_name = reg_string(
            HKEY_LOCAL_MACHINE,
            w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
            w!("ProcessorNameString"),
        );
        let key = w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        let build = reg_string(HKEY_LOCAL_MACHINE, key, w!("CurrentBuildNumber"));
        let revision = reg_dword(key, w!("UBR"));
        let release = reg_string(HKEY_LOCAL_MACHINE, key, w!("DisplayVersion"));
        let edition = reg_string(HKEY_LOCAL_MACHINE, key, w!("EditionID"));
        // Avoid manifest-dependent GetVersionEx and stale ProductName on Windows 11.
        let os = build.map(|build| {
            format!(
                "Windows · build {}{}{}{} · {}",
                build,
                revision.map_or(String::new(), |n| format!(".{n}")),
                release.map_or(String::new(), |s| format!(" · {s}")),
                edition.map_or(String::new(), |s| format!(" · {s}")),
                std::env::consts::ARCH
            )
        });
        let logical = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
        let gpu_result = read_gpus();
        Self {
            cpu_name,
            physical_cores: read_cores(),
            logical_processors: (logical > 0).then_some(logical),
            os,
            gpu_error: gpu_result.is_err(),
            gpus: gpu_result.unwrap_or_default(),
        }
    }
}
fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|n| *n == 0).unwrap_or(value.len())])
        .trim()
        .to_owned()
}
fn reg_string(root: HKEY, key: PCWSTR, name: PCWSTR) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut bytes = size_of_val(&buffer) as u32;
    // SAFETY: bounded live UTF-16 buffer, read-only registry API, no HKEY ownership transferred.
    unsafe {
        RegGetValueW(
            root,
            key,
            name,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
    }
    let value = utf16(&buffer);
    (!value.is_empty()).then_some(value)
}
fn reg_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut bytes = 4;
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
    }
    Some(value)
}
fn read_gpus() -> windows::core::Result<Vec<String>> {
    // DXGI factory/adapters are COM RAII values used and released on this worker.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut result = Vec::new();
        for ix in 0..64 {
            let adapter = match factory.EnumAdapters1(ix) {
                Ok(adapter) => adapter,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => return Ok(result),
                Err(e) => return Err(e),
            };
            let desc = adapter.GetDesc1()?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 {
                result.push(format!(
                    "{} [VEN {:04X} DEV {:04X} · LUID {:08X}:{:08X}]",
                    utf16(&desc.Description),
                    desc.VendorId,
                    desc.DeviceId,
                    desc.AdapterLuid.HighPart as u32,
                    desc.AdapterLuid.LowPart
                ));
            }
        }
        Ok(result)
    }
}
fn count_core_records(bytes: &[u8]) -> Option<u32> {
    let mut offset = 0;
    let mut cores = 0;
    while offset < bytes.len() {
        let header = bytes.get(offset..offset.checked_add(8)?)?;
        let relation = i32::from_le_bytes(header[..4].try_into().ok()?);
        let size = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
        if size < 8 {
            return None;
        }
        offset = offset.checked_add(size)?;
        if offset > bytes.len() {
            return None;
        }
        if relation == RelationProcessorCore.0 {
            cores += 1;
        }
    }
    (cores > 0).then_some(cores)
}
fn read_cores() -> Option<u32> {
    let mut bytes = 0;
    unsafe {
        let _ = GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut bytes);
    }
    if bytes == 0 || bytes > 1_048_576 {
        return None;
    }
    // u64 allocation supplies the alignment required by variable-size topology records.
    let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
    let capacity = buffer.len() * 8;
    unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(buffer.as_mut_ptr().cast()),
            &mut bytes,
        )
        .ok()?;
        if bytes as usize > capacity {
            return None;
        }
        count_core_records(std::slice::from_raw_parts(
            buffer.as_ptr().cast(),
            bytes as usize,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Pending,
    NoTarget,
    NoWindow,
    Minimized,
    IdentityChanged,
    QueryFailed,
}
impl Unavailable {
    pub fn label(self, english: bool) -> &'static str {
        match (self, english) {
            (Self::Pending, false) => "等待查询",
            (Self::Pending, true) => "Waiting for query",
            (Self::NoTarget, false) => "未选择目标",
            (Self::NoTarget, true) => "No target selected",
            (Self::NoWindow, false) => "无可见窗口",
            (Self::NoWindow, true) => "No visible window",
            (Self::Minimized, false) => "窗口已最小化",
            (Self::Minimized, true) => "Window minimized",
            (Self::IdentityChanged, false) => "目标身份改变或已退出",
            (Self::IdentityChanged, true) => "Target changed or exited",
            (Self::QueryFailed, false) => "查询不可用",
            (Self::QueryFailed, true) => "Query unavailable",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: Option<u32>,
    pub primary_fallback: bool,
}
impl DisplayInfo {
    pub fn summary(&self, english: bool) -> String {
        let scope = if self.primary_fallback {
            if english { "Primary" } else { "主屏" }
        } else if english {
            "Target display"
        } else {
            "目标屏幕"
        };
        let refresh = self
            .refresh_hz
            .map_or_else(|| "? Hz".into(), |n| format!("{n} Hz"));
        let name = self.name.strip_prefix("\\\\.\\").unwrap_or(&self.name);
        format!("{scope} {name}: {}×{} @ {refresh}", self.width, self.height)
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub inventory: Arc<Inventory>,
    pub target_id: Option<(u32, u64)>,
    pub sampled_at: Option<Instant>,
    pub display: Result<DisplayInfo, Unavailable>,
    pub window: Result<WindowSize, Unavailable>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            inventory: Arc::new(Inventory::default()),
            target_id: None,
            sampled_at: None,
            display: Err(Unavailable::Pending),
            window: Err(Unavailable::NoTarget),
        }
    }
}
impl Snapshot {
    pub fn new() -> Self {
        Self {
            inventory: Arc::new(Inventory::read()),
            ..Default::default()
        }
    }
    pub fn fresh(&self) -> bool {
        self.sampled_at
            .is_some_and(|t| t.elapsed() <= Duration::from_secs(3))
    }
    pub fn select(&mut self, target: Option<&ProcessIdentity>) {
        let id = target.map(ProcessIdentity::id);
        if id != self.target_id {
            self.target_id = id;
            self.sampled_at = None;
            self.display = Err(Unavailable::Pending);
            self.window = Err(if id.is_some() {
                Unavailable::Pending
            } else {
                Unavailable::NoTarget
            });
        }
    }
    pub fn sample(&mut self, target: Option<&ProcessIdentity>) {
        self.select(target);
        let (display, window) = read_dynamic(target);
        self.display = display;
        self.window = window;
        self.sampled_at = Some(Instant::now());
    }
    pub fn window_summary(&self, english: bool) -> String {
        if !self.fresh() {
            return "—".into();
        }
        match self.window {
            Ok(s) => format!("{}×{} px", s.width, s.height),
            Err(e) => e.label(english).into(),
        }
    }
}
struct DpiGuard(DPI_AWARENESS_CONTEXT);
impl DpiGuard {
    fn enter() -> Option<Self> {
        let old =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        (!old.0.is_null()).then_some(Self(old))
    }
}
impl Drop for DpiGuard {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
fn dimensions(rect: RECT) -> Option<WindowSize> {
    let width = rect.right.checked_sub(rect.left)?;
    let height = rect.bottom.checked_sub(rect.top)?;
    (width > 0 && height > 0).then_some(WindowSize {
        width: width as u32,
        height: height as u32,
    })
}
struct WindowSearch {
    pid: u32,
    foreground: HWND,
    best: Option<(HWND, WindowSize, bool)>,
    minimized: bool,
}
unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> BOOL {
    unsafe {
        let state = &mut *(data.0 as *mut WindowSearch);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != state.pid
            || !IsWindowVisible(hwnd).as_bool()
            || GetWindow(hwnd, GW_OWNER)
                .ok()
                .is_some_and(|owner| !owner.is_invalid())
        {
            return true.into();
        }
        if IsIconic(hwnd).as_bool() {
            state.minimized = true;
            return true.into();
        }
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_ok()
            && let Some(size) = dimensions(rect)
        {
            let foreground = hwnd == state.foreground;
            let area = size.width as u64 * size.height as u64;
            if state.best.is_none_or(|(_, old, active)| {
                (foreground, area) > (active, old.width as u64 * old.height as u64)
            }) {
                state.best = Some((hwnd, size, foreground));
            }
        }
        true.into()
    }
}
fn display_info(monitor: HMONITOR, primary: bool) -> Result<DisplayInfo, Unavailable> {
    if monitor.0.is_null() {
        return Err(Unavailable::QueryFailed);
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    unsafe {
        if !GetMonitorInfoW(monitor, (&mut info as *mut MONITORINFOEXW).cast()).as_bool()
            || !EnumDisplaySettingsW(
                PCWSTR(info.szDevice.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            )
            .as_bool()
        {
            return Err(Unavailable::QueryFailed);
        }
    }
    if mode.dmPelsWidth == 0 || mode.dmPelsHeight == 0 {
        return Err(Unavailable::QueryFailed);
    }
    Ok(DisplayInfo {
        name: utf16(&info.szDevice),
        width: mode.dmPelsWidth,
        height: mode.dmPelsHeight,
        refresh_hz: (mode.dmDisplayFrequency > 1).then_some(mode.dmDisplayFrequency),
        primary_fallback: primary,
    })
}
fn valid_target(target: &ProcessIdentity) -> bool {
    processes::identity(target.pid).is_some_and(|p| p.id() == target.id())
}
fn read_dynamic(
    target: Option<&ProcessIdentity>,
) -> (
    Result<DisplayInfo, Unavailable>,
    Result<WindowSize, Unavailable>,
) {
    let Some(_dpi) = DpiGuard::enter() else {
        return (Err(Unavailable::QueryFailed), Err(Unavailable::QueryFailed));
    };
    let Some(target) = target else {
        return (
            display_info(
                unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) },
                true,
            ),
            Err(Unavailable::NoTarget),
        );
    };
    if !valid_target(target) {
        return (
            Err(Unavailable::IdentityChanged),
            Err(Unavailable::IdentityChanged),
        );
    }
    let mut search = WindowSearch {
        pid: target.pid,
        foreground: unsafe { GetForegroundWindow() },
        best: None,
        minimized: false,
    };
    let result =
        unsafe { EnumWindows(Some(collect_window), LPARAM(&mut search as *mut _ as isize)) };
    if result.is_err() {
        return (Err(Unavailable::QueryFailed), Err(Unavailable::QueryFailed));
    }
    let Some((hwnd, size, _)) = search.best else {
        let error = if search.minimized {
            Unavailable::Minimized
        } else {
            Unavailable::NoWindow
        };
        return (Err(error), Err(error));
    };
    let display = display_info(
        unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) },
        false,
    );
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid != target.pid || !valid_target(target) {
        return (
            Err(Unavailable::IdentityChanged),
            Err(Unavailable::IdentityChanged),
        );
    }
    (display, Ok(size))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topology_records_are_bounded_and_validated() {
        let mut bytes = Vec::new();
        for _ in 0..3 {
            bytes.extend(0i32.to_le_bytes());
            bytes.extend(8u32.to_le_bytes());
        }
        assert_eq!(count_core_records(&bytes), Some(3));
        assert_eq!(count_core_records(&bytes[..23]), None);
        assert_eq!(count_core_records(&[0; 8]), None);
        assert_eq!(count_core_records(&[]), None);
    }
    #[test]
    fn invalid_rectangles_and_utf16_are_handled() {
        assert_eq!(
            dimensions(RECT {
                left: 10,
                top: 20,
                right: 110,
                bottom: 70
            }),
            Some(WindowSize {
                width: 100,
                height: 50
            })
        );
        assert!(dimensions(RECT::default()).is_none());
        assert_eq!(utf16(&[65, 0, 66]), "A");
    }
    #[test]
    fn target_change_clears_dynamic_data_but_keeps_inventory() {
        let mut s = Snapshot::default();
        let cached = s.inventory.clone();
        s.window = Ok(WindowSize {
            width: 12,
            height: 34,
        });
        s.sampled_at = Some(Instant::now());
        s.select(Some(&ProcessIdentity::new(42, 1, "fixture", None)));
        assert_eq!(s.window, Err(Unavailable::Pending));
        assert!(!s.fresh());
        assert!(Arc::ptr_eq(&cached, &s.inventory));
        s.select(None);
        assert_eq!(s.window, Err(Unavailable::NoTarget));
    }
    #[test]
    fn native_inventory_is_readable_and_missing_target_is_explicit() {
        let mut snapshot = Snapshot::new();
        assert!(snapshot.inventory.cpu_name.is_some());
        assert!(snapshot.inventory.physical_cores.unwrap() > 0);
        assert!(
            snapshot.inventory.logical_processors.unwrap()
                >= snapshot.inventory.physical_cores.unwrap()
        );
        assert!(snapshot.inventory.os.is_some());
        snapshot.sample(None);
        assert_eq!(snapshot.window, Err(Unavailable::NoTarget));
        let mut wrong = processes::identity(std::process::id()).unwrap();
        wrong.started_at += 1;
        snapshot.sample(Some(&wrong));
        assert_eq!(snapshot.window, Err(Unavailable::IdentityChanged));
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SW_MINIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER,
        SetWindowPos, ShowWindow, WS_EX_NOACTIVATE, WS_POPUP, WS_VISIBLE,
    };
    struct OwnedWindow(HWND);
    impl Drop for OwnedWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    #[test]
    fn native_window_resize_minimize_restore_and_close() {
        let _dpi = DpiGuard::enter().unwrap();
        let window = OwnedWindow(unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("NxMonitor device test"),
                WS_POPUP | WS_VISIBLE,
                40,
                40,
                320,
                240,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        });
        let target = processes::identity(std::process::id()).unwrap();
        let mut snapshot = Snapshot::new();
        snapshot.sample(Some(&target));
        assert_eq!(
            snapshot.window,
            Ok(WindowSize {
                width: 320,
                height: 240
            })
        );
        let display = snapshot.display.as_ref().unwrap();
        assert!(!display.primary_fallback);
        assert!(display.width > 0);
        assert!(display.height > 0);
        unsafe {
            SetWindowPos(
                window.0,
                None,
                40,
                40,
                640,
                360,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .unwrap();
        }
        snapshot.sample(Some(&target));
        assert_eq!(
            snapshot.window,
            Ok(WindowSize {
                width: 640,
                height: 360
            })
        );
        unsafe {
            let _ = ShowWindow(window.0, SW_MINIMIZE);
        }
        snapshot.sample(Some(&target));
        assert_eq!(snapshot.window, Err(Unavailable::Minimized));
        unsafe {
            let _ = ShowWindow(window.0, SW_RESTORE);
        }
        snapshot.sample(Some(&target));
        assert_eq!(
            snapshot.window,
            Ok(WindowSize {
                width: 640,
                height: 360
            })
        );
        drop(window);
        snapshot.sample(Some(&target));
        assert_eq!(snapshot.window, Err(Unavailable::NoWindow));
        snapshot.sample(None);
        assert!(snapshot.display.unwrap().primary_fallback);
    }
}
