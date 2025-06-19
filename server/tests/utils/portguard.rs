use std::io::SeekFrom;

use async_fd_lock::{LockWrite, RwLockWriteGuard};
use miette::{Context, IntoDiagnostic, Result};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

const LOCK_PATH: &str = "/var/run/postguard.lock";
const FIRST_PORT: u16 = 10025;

pub struct PortGuard {
    guard: RwLockWriteGuard<File>,
}

impl PortGuard {
    pub async fn lock() -> Result<Self> {
        let guard = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(LOCK_PATH)
            .await
            .into_diagnostic()
            .wrap_err("Failed opening lockfile")?
            .lock_write()
            .await
            .map_err(|e| e.error)
            .into_diagnostic()
            .wrap_err("Failed acquiring PortGuard")?;

        Ok(Self { guard })
    }

    pub async fn port(&mut self) -> Result<u16> {
        let mut buf = String::new();
        self.guard
            .read_to_string(&mut buf)
            .await
            .into_diagnostic()
            .wrap_err("Failed to read lockfile")?;

        let port: u16 = if buf.trim().is_empty() {
            FIRST_PORT
        } else {
            buf.trim()
                .parse()
                .into_diagnostic()
                .wrap_err("Failed to parse lockfile content to port number")?
        };

        self.guard
            .seek(SeekFrom::Start(0))
            .await
            .into_diagnostic()
            .wrap_err("Failed to rewind lockfile")?;

        self.guard
            .write_all(format!("{}\n", port + 1).as_bytes())
            .await
            .into_diagnostic()
            .wrap_err("Failed to write new port number to lockfile")?;

        Ok(port)
    }
}

#[tokio::test]
async fn get_ports() {
    let mut guard_1 = PortGuard::lock().await.expect("Failed locking first guard");
    let port_1 = guard_1
        .port()
        .await
        .expect("Failed retrieving guard_1 port");
    assert!(port_1 >= FIRST_PORT);
    drop(guard_1);

    let mut guard_2 = PortGuard::lock().await.expect("Failed locking first guard");
    let port_2 = guard_2
        .port()
        .await
        .expect("Failed retrieving guard_1 port");
    assert!(port_2 > port_1);
}
