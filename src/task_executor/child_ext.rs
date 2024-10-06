use thiserror::Error;

#[derive(Debug, Error)]
pub enum KillError {
    #[error("Invalid pid: cannot send signal to pid `0`")]
    InvalidPid,
    #[error(
        r#"
    The calling process does not have permission to send the
    signal to any of the target processes.
    "#
    )]
    NoPermission,
    #[error(
        r#"
    The target process or process group does not exist.  Note
    that an existing process might be a zombie, a process that
    has terminated execution, but has not yet been wait(2)ed
    for.
    "#
    )]
    NoWait,
    #[cfg(windows)]
    #[error("An unknown error occurred")]
    Win32Error(u32),
}

pub trait ChildExt {
    fn interrupt(&mut self) -> color_eyre::Result<(), KillError>;
}

impl ChildExt for tokio::process::Child {
    fn interrupt(&mut self) -> color_eyre::Result<(), KillError> {
        let pid = self.id().take();
        match pid {
            Some(pid) => send_ctrl_c(pid),
            None => Err(KillError::InvalidPid),
        }
    }
}

#[cfg(unix)]
pub fn send_ctrl_c(pid: u32) -> color_eyre::Result<(), KillError> {
    if pid == 0 {
        return Err(KillError::InvalidPid);
    }
    let result = unsafe { libc::kill(pid as i32, libc::SIGINT) };
    match result {
        libc::EPERM => Err(KillError::NoPermission),
        libc::ESRCH => Err(KillError::NoWait),
        _ => Ok(()),
    }
}

#[cfg(windows)]
pub fn send_ctrl_c(pid: u32) -> color_eyre::Result<(), KillError> {
    if pid == 0 {
        return Err(KillError::InvalidPid);
    }
    use windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent;
    use windows_sys::Win32::System::Console::CTRL_C_EVENT;
    // impl send_ctrl_c for windows with windows-sys crate
    let result = unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid) };
    match result {
        0 => Ok(()),
        _ => {
            let error = unsafe { GetLastError() };
            Err(KillError::Win32Error(error))
        }
    }
}
