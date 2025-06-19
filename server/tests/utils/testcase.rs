use std::{env, path::Path, process::Stdio};

use async_dropper::{AsyncDrop, AsyncDropper};
use async_trait::async_trait;
use miette::{miette, Context, ErrReport, IntoDiagnostic, Result};
use miltr_server::{Milter, Server};
use once_cell::sync::Lazy;
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    process::Command,
    runtime::{Handle, RuntimeFlavor},
    sync::{Mutex, MutexGuard},
    task::JoinHandle,
};
use tokio_retry::{
    strategy::{jitter, FixedInterval},
    Retry,
};
use tokio_util::compat::TokioAsyncReadCompatExt;

use super::smtpsink::SmtpSink;

static TEST_SERIALIZER: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug)]
pub struct TestCase<M: Milter> {
    inner: Option<InnerTestCase<M>>,
}

impl<M: Milter> Default for TestCase<M> {
    fn default() -> Self {
        Self { inner: None }
    }
}

#[derive(Debug)]
struct InnerTestCase<M: Milter> {
    join_handle: JoinHandle<M>,
    smtp_sink: SmtpSink,
    _guard: MutexGuard<'static, ()>,
}

impl<M: Milter + 'static> TestCase<M> {
    pub async fn setup(mut milter: M, path: &Path) -> Result<AsyncDropper<Self>, ErrReport> {
        // Failsafe to report a nice error to the user
        match Handle::try_current().into_diagnostic().wrap_err("Failed checking current runtime")?.runtime_flavor() {
            RuntimeFlavor::MultiThread => Ok(()),
            _ => Err(
                miette!(
                    help="For these tests to work, we need the '#[tokio::test(flavor = \"multi_thread\")]' attribute",
                    "Unsupported tokio runtime"
                ))
        }?;

        // Lock the guard to be the only test executing
        let guard = TEST_SERIALIZER.lock().await;

        // Ensure postfix is running
        let postfix_status = Command::new("postfix")
            .arg("status")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .into_diagnostic()
            .wrap_err("Failed spawning 'postfix status'")?;
        if !postfix_status.success() {
            Command::new("postfix")
                .arg("start")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .into_diagnostic()
                .wrap_err("Failed running 'postfix start'")?;
        }

        // The milter setup
        let listener = Self::wait_for_socket()
            .await
            .wrap_err("Failed starting tcp listener")?;
        let join_handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("Fail to accept connection");
            let mut socket = socket.compat();

            let mut server = Server::default_postfix(&mut milter);
            let _res = server.handle_connection(&mut socket).await;

            let mut socket = socket.into_inner();
            socket.shutdown().await.expect("Shutdown call failed");

            milter
        });

        // The smtp_sink setup
        let smtp_sink = SmtpSink::setup("setup")
            .await
            .wrap_err("Failed setting up smtpsink")?;

        let inner = InnerTestCase {
            join_handle,
            smtp_sink,
            _guard: guard,
        };

        Ok(AsyncDropper::new(Self { inner: Some(inner) }))
    }

    async fn wait_for_socket() -> Result<TcpListener> {
        let addr = env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let addr_borrow = addr.as_str();

        let retry_strategy = FixedInterval::from_millis(500).map(jitter).take(10);

        let listener = Retry::spawn(retry_strategy, || async move {
            TcpListener::bind(addr_borrow)
                .await
                .into_diagnostic()
                .wrap_err("Fail to bind tcp listener")
        })
        .await?;

        Ok(listener)
    }

    pub async fn as_milter(&mut self) -> Result<M> {
        let Some(ref mut inner) = self.inner else {
            return Err(miette!("This TestCase did not contain an inner"));
        };

        inner.smtp_sink.kill().await;

        let join_handle = &mut inner.join_handle;
        let milter = join_handle
            .await
            .into_diagnostic()
            .wrap_err("Failed awaiting test case join handle")?;

        Ok(milter)
    }
}

#[async_trait]
impl<M: Milter + 'static> AsyncDrop for TestCase<M> {
    async fn async_drop(&mut self) {
        self.as_milter()
            .await
            .expect("Failed awaiting halt of inner milter");
    }
}
