mod cli;
mod execute;

pub use cli::{Cli, Command, FusesCmd, LogCmd, MenuKind};
pub use execute::execute;
