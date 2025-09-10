use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    sync::{Arc, RwLock},
    time::Duration,
};

use lettre::{
    message::header::ContentType,
    transport::smtp::{client::AsyncSmtpConnection, extension::ClientId, response::Response},
    Message,
};
use miette::{miette, Context, Result};
use tokio::{net::TcpListener, time::sleep};

use miltr_server::Milter;

use crate::utils::{run_milter, PostfixInstance};

/// Naive limit on sending mails
///
/// In earlier times, the tests needed retries. Turns out, this was because the
/// milter did not run in parallel but in sequence in the async machinery. Now
/// it seems to be fixed, I leave this here if we need it in the future again.
const MAX_SEND_RETRIES: u64 = 1;
const SMTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RunningState;
pub struct StoppedState;

#[derive(Default, Clone)]
pub struct ConnectCounter {
    inner: Arc<RwLock<usize>>,
}

impl ConnectCounter {
    pub fn get(&self) -> usize {
        *self.inner.read().unwrap()
    }

    pub fn inc(&self) {
        let mut a = self.inner.write().unwrap();
        *a = a.checked_add(1).unwrap();
    }
}

pub struct TestCase<S> {
    postfix: PostfixInstance,
    state: PhantomData<S>,
    connect_count: ConnectCounter,
}

impl<S> TestCase<S> {
    pub fn milter_connections_count(&self) -> usize {
        self.connect_count.get()
    }

    pub async fn create_smtp_transport(&self) -> AsyncSmtpConnection {
        // Open a remote connection to the SMTP relay server
        let client = ClientId::default();
        AsyncSmtpConnection::connect_tokio1(
            ("localhost", self.postfix.port()),
            Some(SMTP_CONNECTION_TIMEOUT),
            &client,
            None,
            None,
        )
        .await
        .expect("Failed to bind to postfix addr")
    }
}

impl TestCase<RunningState> {
    pub async fn setup<M, E>(name: &str, milter: M) -> Result<Self>
    where
        E: Debug + Display + 'static,
        M: Milter<Error = E> + 'static + Clone,
    {
        // Setup and run new milter
        let listener = TcpListener::bind("0.0.0.0:0")
            .await
            .expect("Failed to bind to addr");
        let local_addr = listener.local_addr().expect("Failed to read local addr");
        let connect_count = ConnectCounter::default();
        run_milter(listener, milter, connect_count.clone()).await;

        let postfix = PostfixInstance::setup(name, local_addr)
            .await
            .wrap_err("Failed setting up postfix")?;

        Ok(Self {
            postfix,
            state: PhantomData,
            connect_count,
        })
    }

    pub async fn send_mail_with_transport(
        &self,
        transport: &mut AsyncSmtpConnection,
    ) -> Result<Response, lettre::transport::smtp::Error> {
        // Send a mail through postfix with existing smtp connection
        let email = Message::builder()
            .from(
                "NoBody <nobody@domain.tld>"
                    .parse()
                    .expect("Failed mail addr parsing"),
            )
            .to("Hei <hei@domain.tld>"
                .parse()
                .expect("Failed mail addr parsing"))
            .subject("Happy new year")
            .header(ContentType::TEXT_PLAIN)
            .body(String::from("Be happy!"))
            .expect("Failed mail building");
        let envelope = email.envelope();
        let raw = email.formatted();

        println!("Sending mail to port {}", self.postfix.port());

        // We retry the send here as there is enough networking involved for this
        // send to fail spuriously. Either postfix is not ready yet or the backend
        // connection between postfix and the milter does not work yet.
        let mut retry_counter: u64 = 0;
        loop {
            // Send the email
            println!("\tAttemtp '{}' sending mail", retry_counter + 1);
            match transport.send(envelope, &raw).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if retry_counter >= MAX_SEND_RETRIES || transport.has_broken() {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(retry_counter * 500)).await;
                    retry_counter += 1;
                }
            }
        }
    }

    pub async fn send_mail(&self) -> Result<Response, lettre::transport::smtp::Error> {
        let mut transport = self.create_smtp_transport().await;
        let result = self.send_mail_with_transport(&mut transport).await;
        result
    }

    pub async fn stop(self) -> Result<TestCase<StoppedState>> {
        self.postfix.stop().await?;

        Ok(TestCase {
            postfix: self.postfix,
            state: PhantomData,
            connect_count: self.connect_count,
        })
    }
}

impl TestCase<StoppedState> {
    pub async fn log_file_content(&self) -> Result<String> {
        self.postfix.log_file_content().await
    }

    pub async fn validate_mail(&self, needle: &str, response: &Response) -> Result<String> {
        let content = self.postfix.get_mail_content(response).await?;

        if content.contains(needle) {
            Ok(content)
        } else {
            Err(miette!(
                "Email is not correctly modified by Milter, needle '{}' not found in '{}'",
                needle,
                content
            ))
        }
    }
}
