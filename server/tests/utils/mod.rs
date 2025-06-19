mod portguard;
pub mod smtpsink;
pub mod testcase;

pub use portguard::PortGuard;

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::{fs, process::Command};

use tokio_retry::{
    strategy::{jitter, FixedInterval},
    Retry,
};

/// Remove content of a directory
pub async fn remove_dir_contents<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut read_dir = fs::read_dir(path).await.into_diagnostic()?;
    while let Some(entry) = read_dir.next_entry().await.into_diagnostic()? {
        fs::remove_file(entry.path()).await.into_diagnostic()?;
    }
    Ok(())
}

pub async fn send_mail(rcpts: &str) -> Result<()> {
    //Send_mail()
    let _command = Command::new("swaks")
        .args([
            "-t",
            rcpts,
            "-f",
            "monitoring@blackhole.com",
            "--server",
            "127.0.0.1:25",
        ])
        .stdout(Stdio::null())
        .spawn()
        .into_diagnostic()
        .wrap_err("swaks failed to start")?
        .wait()
        .await
        .into_diagnostic()
        .wrap_err("Swaks failed to send mail")?;

    Ok(())
}

pub async fn wait_for_file(path: &Path) -> Result<PathBuf> {
    let retry_strategy = FixedInterval::from_millis(500).map(jitter).take(10);

    let res = Retry::spawn(retry_strategy, || async move { try_fetch_file(path).await })
        .await
        .wrap_err("Awaiting file in output dir timed out")?;

    Ok(res)
}

async fn try_fetch_file(path: &Path) -> Result<PathBuf> {
    //Find the latest added file in /workspace/emails
    let mut entries = fs::read_dir(path)
        .await
        .into_diagnostic()
        .wrap_err("Failed to read directory")?;

    let file = entries
        .next_entry()
        .await
        .into_diagnostic()
        .wrap_err("Failed fetching first file")?
        .ok_or(miette!("No file found in watched dir"))?;

    Ok(file.path())
}
