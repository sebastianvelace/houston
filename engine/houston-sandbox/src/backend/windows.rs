//! Windows sandbox backend: Job Object process isolation.

use super::{SandboxBackend, SandboxCapabilities};
use crate::SandboxError;
use houston_policy::SessionPolicy;
use tokio::process::Command;

pub struct WindowsBackend;

impl SandboxBackend for WindowsBackend {
    fn wrap_command(&self, mut cmd: Command, _policy: &SessionPolicy) -> Result<Command, SandboxError> {
        let strict = crate::sandbox_strict();
        unsafe {
            cmd.pre_exec(move || install_job_object(strict));
        }
        Ok(cmd)
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            filesystem_isolation: false,
            network_isolation: false,
            fd_cleanup: false,
            credential_isolation: true,
            platform: "windows-job",
        }
    }
}

unsafe fn install_job_object(strict: bool) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let job = CreateJobObjectW(std::ptr::null(), std::ptr::null(), std::ptr::null());
    if job.is_null() {
        let err = Error::last_os_error();
        if strict {
            return Err(err);
        }
        return Ok(());
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
        let err = Error::last_os_error();
        CloseHandle(job);
        if strict {
            return Err(err);
        }
        return Ok(());
    }

    let ok = AssignProcessToJobObject(job, GetCurrentProcess(), 0);
    if ok == 0 {
        let err = Error::last_os_error();
        CloseHandle(job);
        if strict {
            return Err(err);
        }
        return Ok(());
    }

    // Keep the job handle alive for the process lifetime.
    std::mem::forget(job);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use houston_policy::SessionPolicy;
    use std::path::PathBuf;

    #[test]
    fn strict_mode_wraps_without_error() {
        std::env::set_var("HOUSTON_SANDBOX", "strict");
        let cmd = Command::new("cmd");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("C:\\agent"), None);
        WindowsBackend.wrap_command(cmd, &policy).expect("strict wrap ok");
        std::env::remove_var("HOUSTON_SANDBOX");
    }

    #[test]
    fn permissive_mode_allows_spawn() {
        std::env::set_var("HOUSTON_SANDBOX", "permissive");
        let cmd = Command::new("cmd");
        let policy = SessionPolicy::for_working_dir(PathBuf::from("C:\\agent"), None);
        WindowsBackend.wrap_command(cmd, &policy).expect("permissive ok");
        std::env::remove_var("HOUSTON_SANDBOX");
    }

    #[test]
    fn capabilities_report_job_backend() {
        let caps = WindowsBackend.capabilities();
        assert_eq!(caps.platform, "windows-job");
        assert!(caps.credential_isolation);
    }
}
