//! Windows physical memory and target working set. Called only by the runtime worker.
//! No VM_READ, elevation, remote allocation or target modification is required.
use nxmonitor::target::ProcessIdentity;
use std::{
    collections::VecDeque,
    mem::size_of,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{CloseHandle, FILETIME, GetLastError, HANDLE},
    System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

pub const HISTORY_LIMIT: usize = 60;
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
pub const STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Pending,
    NoTarget,
    AccessDenied,
    Exited,
    IdentityChanged,
    QueryFailed(u32),
    InvalidData,
}
impl Unavailable {
    pub fn label(self, english: bool) -> &'static str {
        match (self, english) {
            (Self::Pending, false) => "等待采样",
            (Self::Pending, true) => "Waiting for sample",
            (Self::NoTarget, false) => "未选择目标",
            (Self::NoTarget, true) => "No target selected",
            (Self::AccessDenied, false) => "无读取权限",
            (Self::AccessDenied, true) => "Access denied",
            (Self::Exited, false) => "目标已退出",
            (Self::Exited, true) => "Target exited",
            (Self::IdentityChanged, false) => "进程身份改变",
            (Self::IdentityChanged, true) => "Process identity changed",
            (Self::QueryFailed(_), false) => "内存查询失败",
            (Self::QueryFailed(_), true) => "Memory query failed",
            (Self::InvalidData, false) => "内存数据无效",
            (Self::InvalidData, true) => "Invalid memory data",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalMemory {
    pub used_bytes: u64,
    pub total_bytes: u64,
}
impl PhysicalMemory {
    fn from_available(total_bytes: u64, available_bytes: u64) -> Result<Self, Unavailable> {
        if total_bytes == 0 {
            return Err(Unavailable::InvalidData);
        }
        let used_bytes = total_bytes
            .checked_sub(available_bytes)
            .ok_or(Unavailable::InvalidData)?;
        Ok(Self {
            used_bytes,
            total_bytes,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub target_id: Option<(u32, u64)>,
    pub sampled_at: Option<Instant>,
    pub system: Result<PhysicalMemory, Unavailable>,
    pub working_set_bytes: Result<u64, Unavailable>,
    /// One entry per actual sampling attempt; None is a gap, never a fabricated zero.
    pub system_history: VecDeque<Option<u64>>,
    pub process_history: VecDeque<Option<u64>>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            target_id: None,
            sampled_at: None,
            system: Err(Unavailable::Pending),
            working_set_bytes: Err(Unavailable::NoTarget),
            system_history: VecDeque::new(),
            process_history: VecDeque::new(),
        }
    }
}
impl Snapshot {
    pub fn fresh(&self) -> bool {
        self.sampled_at
            .is_some_and(|time| time.elapsed() <= STALE_AFTER)
    }
    pub fn select(&mut self, target: Option<&ProcessIdentity>) {
        let id = target.map(ProcessIdentity::id);
        if self.target_id != id {
            self.target_id = id;
            self.working_set_bytes = Err(if id.is_some() {
                Unavailable::Pending
            } else {
                Unavailable::NoTarget
            });
            self.process_history.clear();
        }
    }
    fn record(
        &mut self,
        system: Result<PhysicalMemory, Unavailable>,
        process: Result<u64, Unavailable>,
    ) {
        self.sampled_at = Some(Instant::now());
        self.system = system;
        self.working_set_bytes = process;
        push_bounded(&mut self.system_history, system.ok().map(|s| s.used_bytes));
        if self.target_id.is_some() {
            push_bounded(&mut self.process_history, process.ok());
        }
    }
    pub fn sample(&mut self, target: Option<&ProcessIdentity>) {
        self.select(target);
        self.record(
            read_system(),
            target.map_or(Err(Unavailable::NoTarget), read_working_set),
        );
    }
}
fn push_bounded(history: &mut VecDeque<Option<u64>>, value: Option<u64>) {
    if history.len() == HISTORY_LIMIT {
        history.pop_front();
    }
    history.push_back(value);
}
pub fn mib(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}
pub fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

fn last_error() -> Unavailable {
    // Capture immediately after the failing Win32 call, before any other OS operation.
    match unsafe { GetLastError().0 } {
        5 => Unavailable::AccessDenied,
        87 | 1168 => Unavailable::Exited,
        code => Unavailable::QueryFailed(code),
    }
}
pub fn read_system() -> Result<PhysicalMemory, Unavailable> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: initialized, correctly sized output struct lives through this synchronous call.
    unsafe {
        GlobalMemoryStatusEx(&mut status).map_err(|_| last_error())?;
    }
    PhysicalMemory::from_available(status.ullTotalPhys, status.ullAvailPhys)
}
struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn validate_identity(handle: HANDLE, expected: &ProcessIdentity) -> Result<(), Unavailable> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user)
            .map_err(|_| last_error())?;
    }
    let actual = (created.dwHighDateTime as u64) << 32 | created.dwLowDateTime as u64;
    if actual != expected.started_at {
        return Err(Unavailable::IdentityChanged);
    }
    if exited.dwHighDateTime != 0 || exited.dwLowDateTime != 0 {
        return Err(Unavailable::Exited);
    }
    Ok(())
}
pub fn read_working_set(target: &ProcessIdentity) -> Result<u64, Unavailable> {
    // SAFETY: limited-query owned handle, RAII closed; no write access to target memory.
    let handle = ProcessHandle(unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, target.pid)
            .map_err(|_| last_error())?
    });
    validate_identity(handle.0, target)?;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    unsafe {
        if !K32GetProcessMemoryInfo(
            handle.0,
            &mut counters,
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .as_bool()
        {
            return Err(last_error());
        }
    }
    validate_identity(handle.0, target)?; // The target may have exited while querying.
    Ok(counters.WorkingSetSize as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(pid: u32, start: u64) -> ProcessIdentity {
        ProcessIdentity::new(pid, start, "fixture.exe", None)
    }
    #[test]
    fn physical_used_and_binary_units_are_precise() {
        assert_eq!(
            PhysicalMemory::from_available(8 * 1_073_741_824, 3 * 1_073_741_824)
                .unwrap()
                .used_bytes,
            5 * 1_073_741_824
        );
        assert_eq!(mib(1_572_864), 1.5);
        assert_eq!(gib(1_610_612_736), 1.5);
        assert_eq!(
            PhysicalMemory::from_available(0, 0),
            Err(Unavailable::InvalidData)
        );
        assert_eq!(
            PhysicalMemory::from_available(1, 2),
            Err(Unavailable::InvalidData)
        );
    }
    #[test]
    fn target_change_and_pid_reuse_clear_only_process_history() {
        let mut s = Snapshot::default();
        s.select(Some(&target(10, 1)));
        s.record(PhysicalMemory::from_available(8, 3), Ok(4));
        s.select(Some(&target(10, 1)));
        assert_eq!(s.process_history.len(), 1);
        s.select(Some(&target(10, 2)));
        assert!(s.process_history.is_empty());
        assert_eq!(s.working_set_bytes, Err(Unavailable::Pending));
        assert_eq!(s.system_history.len(), 1);
        s.select(None);
        assert_eq!(s.working_set_bytes, Err(Unavailable::NoTarget));
    }
    #[test]
    fn errors_replace_live_values_and_leave_history_gaps() {
        let mut s = Snapshot::default();
        s.select(Some(&target(10, 1)));
        s.record(PhysicalMemory::from_available(8, 3), Ok(4));
        s.record(
            Err(Unavailable::QueryFailed(1)),
            Err(Unavailable::AccessDenied),
        );
        assert_eq!(s.system, Err(Unavailable::QueryFailed(1)));
        assert_eq!(s.working_set_bytes, Err(Unavailable::AccessDenied));
        assert_eq!(s.process_history.back(), Some(&None));
        assert_eq!(s.system_history.back(), Some(&None));
    }
    #[test]
    fn history_and_freshness_are_bounded() {
        let mut s = Snapshot::default();
        s.select(Some(&target(10, 1)));
        for n in 0..100 {
            s.record(PhysicalMemory::from_available(1000, n), Ok(n));
        }
        assert_eq!(s.system_history.len(), HISTORY_LIMIT);
        assert_eq!(s.process_history.len(), HISTORY_LIMIT);
        assert_eq!(s.process_history.front(), Some(&Some(40)));
        assert!(s.fresh());
        s.sampled_at = Some(Instant::now() - Duration::from_secs(4));
        assert!(!s.fresh());
    }
    #[test]
    fn native_read_and_wrong_identity() {
        let process =
            super::super::processes::identity(std::process::id()).expect("self query allowed");
        let system = read_system().unwrap();
        assert!(system.total_bytes > 0);
        assert!(system.used_bytes <= system.total_bytes);
        assert!(read_working_set(&process).unwrap() > 0);
        let mut wrong = process;
        wrong.started_at += 1;
        assert_eq!(read_working_set(&wrong), Err(Unavailable::IdentityChanged));
    }
    #[test]
    fn native_exited_process_is_unavailable() {
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/C", "ping -n 3 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        let process = super::super::processes::identity(child.id()).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(matches!(
            read_working_set(&process),
            Err(Unavailable::Exited)
        ));
    }
}
