use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    jj_axi::run_from(std::env::args_os(), &cwd).await
}
