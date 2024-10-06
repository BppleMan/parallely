pub mod child_ext;

use crate::task_executor::child_ext::ChildExt;
use color_eyre::eyre::eyre;
use std::fmt::{Display, Formatter};
use std::process::ExitStatus;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

pub type TaskOutputReceiver = mpsc::UnboundedReceiver<String>;

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Ready(String),
    Executing {
        command: String,
        pid: Option<u32>,
    },
    Killed {
        command: String,
        pid: Option<u32>,
    },
    Exited {
        command: String,
        pid: Option<u32>,
        status: ExitStatus,
    },
}

impl Display for TaskStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Ready(command) => {
                write!(f, "Ready: {}", command)
            }
            TaskStatus::Executing { command, pid } => {
                write!(f, "Executing: {} (PID: {})", command, pid.unwrap_or(0))
            }
            TaskStatus::Killed { command, pid } => {
                write!(f, "Killed: {} (PID: {})", command, pid.unwrap_or(0))
            }
            TaskStatus::Exited {
                command,
                pid,
                status,
            } => {
                write!(
                    f,
                    "Exited: {} (PID: {}) with status: {}",
                    command,
                    pid.unwrap_or(0),
                    status
                )
            }
        }
    }
}

#[allow(unused)]
pub trait Executable {
    fn raw_command(&self) -> &str;

    fn pid(&self) -> Option<u32>;

    fn try_wait(&mut self) -> color_eyre::Result<TaskStatus>;

    async fn wait(&mut self) -> color_eyre::Result<TaskStatus>;

    async fn interrupt(&mut self) -> color_eyre::Result<()>;

    async fn interrupt_and_wait(&mut self) -> color_eyre::Result<TaskStatus> {
        self.interrupt().await?;
        self.wait().await
    }

    async fn kill(&mut self) -> color_eyre::Result<()>;

    async fn kill_and_wait(&mut self) -> color_eyre::Result<TaskStatus> {
        self.kill().await?;
        self.wait().await
    }
}

pub struct TaskExecutor {
    pub command: Command,
    raw_command: String,
    child: Option<Child>,
    pid: Option<u32>,
    shutdown_sender: Option<oneshot::Sender<()>>,
}

impl TaskExecutor {
    pub fn new(raw_command: String) -> Self {
        let mut args = raw_command.split_whitespace().collect::<Vec<_>>();
        let mut command = Command::new(args.remove(0));
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        Self {
            command,
            raw_command,
            child: None,
            pid: None,
            shutdown_sender: None,
        }
    }

    pub fn execute(&mut self) -> color_eyre::Result<TaskOutputReceiver> {
        let (shutdown_sender, mut shutdown_receiver) = oneshot::channel();
        let (output_sender, output_receiver) = mpsc::unbounded_channel();
        let mut child = self.command.spawn()?;
        let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
        self.child = Some(child);
        self.pid = self.child.as_ref().unwrap().id();
        self.shutdown_sender = Some(shutdown_sender);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_receiver => {
                        break;
                    }
                    Ok(line) = stdout.next_line() => {
                        match line {
                            Some(line) => {
                                if output_sender.send(line).is_err() {
                                    break;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    Ok(line) = stderr.next_line() => {
                        match line {
                            Some(line) => {
                                if output_sender.send(line).is_err() {
                                    break;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }
            tracing::debug!("TaskExecutor::execute - end");
        });
        Ok(output_receiver)
    }
}

impl Executable for TaskExecutor {
    fn raw_command(&self) -> &str {
        self.raw_command.as_str()
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }

    fn try_wait(&mut self) -> color_eyre::Result<TaskStatus> {
        if let Some(child) = self.child.as_mut() {
            let result = child.try_wait().map(|status| {
                status
                    .map(|status| TaskStatus::Exited {
                        command: self.raw_command.clone(),
                        pid: self.pid(),
                        status,
                    })
                    .unwrap_or(TaskStatus::Executing {
                        command: self.raw_command.clone(),
                        pid: self.pid(),
                    })
            })?;
            Ok(result)
        } else {
            Ok(TaskStatus::Ready(self.raw_command.clone()))
        }
    }

    async fn wait(&mut self) -> color_eyre::Result<TaskStatus> {
        if let Some(child) = self.child.as_mut() {
            let result = child.wait().await?;
            Ok(TaskStatus::Exited {
                command: self.raw_command.clone(),
                pid: self.pid(),
                status: result,
            })
        } else {
            Ok(TaskStatus::Ready(self.raw_command.clone()))
        }
    }

    async fn interrupt(&mut self) -> color_eyre::Result<()> {
        if let (Some(child), Some(sender)) = (self.child.as_mut(), self.shutdown_sender.take()) {
            if sender.send(()).is_err() {
                return Err(eyre!("Failed to send shutdown signal"));
            }
            child.interrupt()?;
        }
        Ok(())
    }

    async fn kill(&mut self) -> color_eyre::Result<()> {
        if let (Some(child), Some(sender)) = (self.child.as_mut(), self.shutdown_sender.take()) {
            if sender.send(()).is_err() {
                return Err(eyre!("Failed to send shutdown signal"));
            }
            child.kill().await?;
        }
        Ok(())
    }
}
