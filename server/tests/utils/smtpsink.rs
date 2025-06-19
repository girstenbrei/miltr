use std::path::Path;

use miette::{Context, IntoDiagnostic, Result};
use tokio::process::{Child, Command};

use crate::utils::remove_dir_contents;

#[derive(Debug)]
pub struct SmtpSink {
    child: Child,
}

impl SmtpSink {
    pub async fn setup(path: &Path) -> Result<SmtpSink> {
        // Create the tempdir smtpsink will be running in
        tokio::fs::create_dir_all(path)
            .await
            .into_diagnostic()
            .wrap_err("Failed to create directory for smtpsink")?;

        // Ensure it is empty
        remove_dir_contents(path)
            .await
            .wrap_err("Failed to empty directory")?;

        // Run smtpsink
        let child = Command::new("smtp-sink")
            .current_dir(path)
            .args(["-u", "root", "-d", "-c", "127.0.0.1:2525", "100"])
            .spawn()
            .into_diagnostic()
            .wrap_err("Failed spawning smtp_sink command")?;

        Ok(Self { child })
    }

    pub async fn kill(&mut self) {
        self.child.kill().await.expect("Failed killing smtp-sink");
    }
}
