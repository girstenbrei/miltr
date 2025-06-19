use std::path::{Path, PathBuf};

use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::{
    fs,
    process::{Child, Command},
};
use tokio_retry::{
    strategy::{jitter, FixedInterval},
    Retry,
};

use crate::utils::PortGuard;

#[derive(Debug)]
pub struct SmtpSink {
    child: Child,
    sink_dir: PathBuf,
    pub port: u16,
}

impl SmtpSink {
    pub async fn setup(name: &str) -> Result<SmtpSink> {
        let sink_dir = PathBuf::from(format!("/var/smtp-sink/{name}"));

        // Create the tempdir smtpsink will be running in
        tokio::fs::create_dir_all(&sink_dir)
            .await
            .into_diagnostic()
            .wrap_err("Failed to create directory for smtpsink")?;

        // Ensure it is empty
        remove_dir_contents(&sink_dir)
            .await
            .wrap_err("Failed to empty directory")?;

        // Retrieve a new port for this instance
        let port = PortGuard::lock()
            .await
            .wrap_err("Failed locking portguard from smtp-sink")?
            .port()
            .await
            .wrap_err("Failed retrieving port for smtp-sink")?;

        // Run smtpsink
        let child = Command::new("smtp-sink")
            .current_dir(&sink_dir)
            .args([
                "-u",
                "root",
                "-d",
                "-c",
                &format!("127.0.0.1:{port}"),
                "100",
            ])
            .spawn()
            .into_diagnostic()
            .wrap_err("Failed spawning smtp_sink command")?;

        Ok(Self {
            child,
            sink_dir,
            port,
        })
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await.into_diagnostic()
    }

    pub async fn wait_for_file(&self) -> Result<PathBuf> {
        let retry_strategy = FixedInterval::from_millis(500).map(jitter).take(20);

        let res = Retry::spawn(retry_strategy, || async move {
            try_fetch_file(&self.sink_dir).await
        })
        .await
        .wrap_err("Awaiting file in output dir timed out")?;

        Ok(res)
    }
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

/// Remove content of a directory
pub async fn remove_dir_contents<P: AsRef<Path>>(path: P) -> Result<()> {
    let mut read_dir = fs::read_dir(path).await.into_diagnostic()?;
    while let Some(entry) = read_dir.next_entry().await.into_diagnostic()? {
        fs::remove_file(entry.path()).await.into_diagnostic()?;
    }
    Ok(())
}
