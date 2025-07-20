use std::io::SeekFrom;

use async_fd_lock::{LockWrite, RwLockWriteGuard};
use miette::{Context, IntoDiagnostic, Result};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

const LOCK_PATH: &str = "/var/run/postguard.lock";
const FIRST_PORT: u16 = 10025;

/// Multi-Process safe port number generator
///
/// Running multiple server instances that need non-colliding port numbers
/// to connect to can request a single port via `PortGuard::port()`.
///
/// This is just a disk-based advisory file lock storing the last port returned.
pub struct PortGuard {
    guard: RwLockWriteGuard<File>,
}

impl PortGuard {
    async fn lock() -> Result<Self> {
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

    pub async fn port() -> Result<u16> {
        let mut this = PortGuard::lock()
            .await
            .wrap_err("Failed locking postguard")?;

        let mut buf = String::new();
        this.guard
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

        this.guard
            .seek(SeekFrom::Start(0))
            .await
            .into_diagnostic()
            .wrap_err("Failed to rewind lockfile")?;

        this.guard
            .write_all(format!("{}\n", port + 1).as_bytes())
            .await
            .into_diagnostic()
            .wrap_err("Failed to write new port number to lockfile")?;

        Ok(port)
    }
}

#[tokio::test]
async fn get_ports() {
    let port_1 = PortGuard::port()
        .await
        .expect("Failed retrieving guard_1 port");
    assert!(port_1 >= FIRST_PORT);

    let port_2 = PortGuard::port()
        .await
        .expect("Failed retrieving guard_1 port");
    assert!(port_2 > port_1);
}
