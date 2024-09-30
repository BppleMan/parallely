use clap::Parser;
use std::fmt::{Debug, Display, Formatter};
use std::process::ExitStatus;

#[derive(Default, Debug, Parser)]
#[command(version, about, author)]
pub struct Parallely {
    /// The commands to run in parallel. e.g. `parallely -c echo hello -c echo world`
    #[arg(short, long)]
    pub commands: Vec<String>,

    /// Exit on all sub-processes complete.
    #[arg(long = "eoc")]
    pub exit_on_complete: bool,

    /// Write log into $(PWD)/logs.
    #[arg(short, long)]
    pub debug: bool,
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
