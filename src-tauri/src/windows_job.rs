#![cfg(windows)]

use std::io;
use std::mem::size_of;
#[cfg(test)]
use std::os::windows::io::AsRawHandle;
#[cfg(test)]
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
#[cfg(test)]
use windows_sys::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};

#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

// Kernel object handles are safe to move between threads. Ownership remains unique.
unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    pub fn kill_on_close() -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            unsafe { CloseHandle(handle) };
            return Err(io::Error::last_os_error());
        }

        Ok(Self { handle })
    }

    #[cfg(test)]
    pub fn assign<T: AsRawHandle>(&self, process: &T) -> io::Result<()> {
        self.assign_raw(process.as_raw_handle() as HANDLE)
    }

    pub fn assign_raw(&self, process: HANDLE) -> io::Result<()> {
        let assigned = unsafe { AssignProcessToJobObject(self.handle, process) };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.handle, exit_code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
pub fn process_is_running(pid: u32) -> bool {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let result = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    unsafe { CloseHandle(handle) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    fn direct_children(parent_pid: u32) -> Vec<u32> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut children = Vec::new();
        let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        while has_entry {
            if entry.th32ParentProcessID == parent_pid {
                children.push(entry.th32ProcessID);
            }
            has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };
        children
    }

    #[test]
    fn kill_on_close_terminates_process_tree() {
        let mut child = Command::new("cmd.exe")
            .args(["/d", "/c", "ping.exe", "-t", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn process tree fixture");
        let root_pid = child.id();
        let job = JobObject::kill_on_close().expect("create kill-on-close job");
        job.assign(&child).expect("assign fixture to job");

        let deadline = Instant::now() + Duration::from_secs(3);
        let descendants = loop {
            let found = direct_children(root_pid);
            if !found.is_empty() || Instant::now() >= deadline {
                break found;
            }
            thread::sleep(Duration::from_millis(50));
        };
        assert!(
            !descendants.is_empty(),
            "fixture did not create a child process"
        );

        drop(job);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline
            && (process_is_running(root_pid) || descendants.iter().copied().any(process_is_running))
        {
            thread::sleep(Duration::from_millis(50));
        }

        let _ = child.wait();
        assert!(!process_is_running(root_pid));
        assert!(descendants.into_iter().all(|pid| !process_is_running(pid)));
    }
}
