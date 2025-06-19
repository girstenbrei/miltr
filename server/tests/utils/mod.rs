mod portguard;
pub mod smtpsink;
pub mod testcase;

pub use portguard::PortGuard;
pub use smtpsink::remove_dir_contents;

use std::process::Stdio;

use miette::{Context, IntoDiagnostic, Result};
use tokio::process::Command;

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
