mod cli;
mod execute;
pub mod gen_all;

pub use cli::{Cli, Command, FusesCmd, GenCmd, LogCmd};
pub use execute::execute;
