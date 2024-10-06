use crate::shutdown_handler::ShutdownReason;
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

#[derive(Debug)]
pub enum ChildSignal {
    Interrupt,
    Quit,
    Terminate,
}

#[cfg(unix)]
impl From<ChildSignal> for libc::c_int {
    fn from(signal: ChildSignal) -> Self {
        match signal {
            ChildSignal::Interrupt => libc::SIGINT,
            ChildSignal::Quit => libc::SIGQUIT,
            ChildSignal::Terminate => libc::SIGTERM,
        }
    }
}

impl From<ShutdownReason> for ChildSignal {
    fn from(reason: ShutdownReason) -> Self {
        match reason {
            ShutdownReason::CtrlC => ChildSignal::Interrupt,
            ShutdownReason::Quit => ChildSignal::Quit,
            ShutdownReason::Sigint => ChildSignal::Interrupt,
            ShutdownReason::Sigterm => ChildSignal::Terminate,
            ShutdownReason::Sigquit => ChildSignal::Quit,
            ShutdownReason::End => ChildSignal::Terminate,
        }
    }
}

#[allow(unused)]
pub trait ChildExt {
    fn send_signal(&self, signal: ChildSignal) -> color_eyre::Result<(), KillError>;

    fn interrupt(&self) -> color_eyre::Result<(), KillError> {
        self.send_signal(ChildSignal::Interrupt)
    }

    fn quit(&self) -> color_eyre::Result<(), KillError> {
        self.send_signal(ChildSignal::Quit)
    }

    fn terminate(&self) -> color_eyre::Result<(), KillError> {
        self.send_signal(ChildSignal::Terminate)
    }
}

impl ChildExt for tokio::process::Child {
    #[cfg(unix)]
    fn send_signal(&self, signal: ChildSignal) -> color_eyre::Result<(), KillError> {
        let pid = self.id().take();
        match pid {
            Some(0) | None => Err(KillError::InvalidPid),
            Some(pid) => {
                let result = unsafe { libc::kill(pid as i32, signal.into()) };
                match result {
                    libc::EPERM => Err(KillError::NoPermission),
                    libc::ESRCH => Err(KillError::NoWait),
                    _ => Ok(()),
                }
            }
        }
    }

    #[cfg(windows)]
    fn send_signal(&self, signal: ChildSignal) -> color_eyre::Result<(), KillError> {
        let pid = self.id().take();
        match pid {
            Some(0) | None => Err(KillError::InvalidPid),
            Some(pid) => match signal {
                ChildSignal::Interrupt => {
                    use windows_sys::Win32::Foundation::GetLastError;
                    use windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent;
                    use windows_sys::Win32::System::Console::CTRL_C_EVENT;
                    let result = unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid) };
                    match result {
                        0 => Ok(()),
                        _ => {
                            let error = unsafe { GetLastError() };
                            Err(KillError::Win32Error(error))
                        }
                    }
                }
                ChildSignal::Quit | ChildSignal::Terminate => {
                    use windows_sys::Win32::Foundation::GetLastError;
                    use windows_sys::Win32::Foundation::FALSE;
                    use windows_sys::Win32::System::Threading::OpenProcess;
                    use windows_sys::Win32::System::Threading::TerminateProcess;
                    use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
                    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, pid) };
                    if handle.is_null() {
                        let error = unsafe { GetLastError() };
                        return Err(KillError::Win32Error(error));
                    }
                    let result = unsafe { TerminateProcess(handle, 1) };
                    match result {
                        0 => Ok(()),
                        _ => {
                            let error = unsafe { GetLastError() };
                            Err(KillError::Win32Error(error))
                        }
                    }
                }
            },
        }
    }
}
