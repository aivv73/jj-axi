#![forbid(unsafe_code)]

mod cli;
mod commands;
mod error;
mod jj_bridge;
mod model;
mod toon;

use std::path::Path;

pub async fn run_from<I, T>(args: I, cwd: &Path) -> std::process::ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    commands::run(cli::parse(args), cwd).await
}
