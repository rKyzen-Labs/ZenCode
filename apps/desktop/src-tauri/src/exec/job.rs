use std::process::Command;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    TerminateJobObject, JobObjectExtendedLimitInformation,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

pub struct ProcessJob {
    #[cfg(windows)]
    job_handle: Option<HANDLE>,
    pid: Option<u32>,
}

unsafe impl Send for ProcessJob {}
unsafe impl Sync for ProcessJob {}

impl ProcessJob {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Self {
                        job_handle: None,
                        pid: None,
                    };
                }

                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );

                if ok == 0 {
                    CloseHandle(job);
                    Self {
                        job_handle: None,
                        pid: None,
                    }
                } else {
                    Self {
                        job_handle: Some(job),
                        pid: None,
                    }
                }
            }
        }
        #[cfg(not(windows))]
        {
            Self { pid: None }
        }
    }

    pub fn assign_process(&mut self, pid: u32) {
        self.pid = Some(pid);
        #[cfg(windows)]
        {
            if let Some(job) = self.job_handle {
                unsafe {
                    let proc = OpenProcess(
                        PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                        0,
                        pid,
                    );
                    if !proc.is_null() {
                        let _ = AssignProcessToJobObject(job, proc);
                        CloseHandle(proc);
                    }
                }
            }
        }
    }

    pub fn terminate(&mut self) {
        if let Some(pid) = self.pid {
            Self::kill_tree(pid);
        }

        #[cfg(windows)]
        {
            if let Some(job) = self.job_handle.take() {
                unsafe {
                    let _ = TerminateJobObject(job, 1);
                    CloseHandle(job);
                }
            }
        }
    }

    pub fn kill_tree(pid: u32) {
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
        #[cfg(not(windows))]
        {
            let _ = Command::new("kill")
                .args(["-9", &format!("-{}", pid)])
                .output();
        }
    }
}

impl Drop for ProcessJob {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if let Some(job) = self.job_handle.take() {
                unsafe {
                    CloseHandle(job);
                }
            }
        }
    }
}
