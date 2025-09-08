use std::future::Future;
use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    time::Duration,
};

use lettre::transport::smtp::client::AsyncSmtpConnection;
use lettre::transport::smtp::extension::ClientId;
use lettre::{
    message::header::ContentType, transport::smtp::response::Response, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
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

pub struct RunningState;
pub struct StoppedState;

pub struct TestCase<S> {
    postfix: PostfixInstance,
    state: PhantomData<S>,
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
        run_milter(listener, milter).await;

        let postfix = PostfixInstance::setup(name, local_addr)
            .await
            .wrap_err("Failed setting up postfix")?;

        Ok(Self {
            postfix,
            state: PhantomData,
        })
    }

    pub async fn with_smtp_connection<F, Fut, R>(
        &mut self,
        f: F,
        helo_name: Option<ClientId>,
        timeout: Option<Duration>,
    ) -> R
    where
        F: FnOnce(AsyncSmtpConnection) -> Fut,
        Fut: Future<Output = R> + Send,
    {
        let client = helo_name.unwrap_or_default();
        let connection = AsyncSmtpConnection::connect_tokio1(
            ("localhost", self.postfix.port()),
            timeout,
            &client,
            None,
            None,
        )
        .await
        .expect("Failed to connect to postfix");
        f(connection).await
    }

    pub async fn send_mail(&self) -> Result<Response, lettre::transport::smtp::Error> {
        // Send a mail through postfix
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

        // Open a remote connection to the SMTP relay server
        println!("Sending mail to port {}", self.postfix.port());
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("localhost")
            .port(self.postfix.port())
            .build();

        // We retry the send here as there is enough networking involved for this
        // send to fail spuriously. Either postfix is not ready yet or the backend
        // connection between postfix and the milter does not work yet.
        let mut retry_counter: u64 = 0;
        loop {
            // Send the email
            println!("\tAttemtp '{}' sending mail", retry_counter + 1);
            match mailer.send(email.clone()).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if retry_counter >= MAX_SEND_RETRIES {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(retry_counter * 500)).await;
                    retry_counter += 1;
                }
            }
        }
    }

    pub async fn stop(self) -> Result<TestCase<StoppedState>> {
        self.postfix.stop().await?;

        Ok(TestCase {
            postfix: self.postfix,
            state: PhantomData,
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
