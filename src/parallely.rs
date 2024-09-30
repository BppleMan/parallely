use clap::Parser;
use std::fmt::{Debug, Display, Formatter};
use std::process::ExitStatus;

#[derive(Default, Debug, Parser)]
#[command(version, about, author)]
pub struct Parallely {
    /// The commands to run in parallel. e.g. `parallely -c echo hello -c echo world`
    #[arg(short, long)]
    pub commands: Vec<String>,

    #[arg(long)]
    pub wait: bool,
}

#[derive(Debug)]
pub struct ParallelyResult {
    pub command: String,
    pub exit_status: ExitStatus,
}

impl Display for ParallelyResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]: {}", self.command, self.exit_status)
    }
}
