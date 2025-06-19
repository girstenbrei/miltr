use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    time::Duration,
};

use lettre::{
    message::header::ContentType, transport::smtp::response::Response, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use miette::{miette, Context, IntoDiagnostic, Result};
use miltr_server::Milter;
use tokio::{fs, net::TcpListener, time::sleep};

use crate::utils::{run_milter, PostfixInstance, SmtpSink};

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
    smtp_sink: SmtpSink,
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

        let smtp_sink = SmtpSink::setup(name)
            .await
            .wrap_err("Failed smtp sink setup")?;
        let postfix = PostfixInstance::setup(name, local_addr, smtp_sink.port)
            .await
            .wrap_err("Failed setting up postfix")?;

        Ok(Self {
            postfix,
            smtp_sink,
            state: PhantomData,
        })
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

    pub async fn stop(mut self) -> Result<TestCase<StoppedState>> {
        self.postfix.stop().await?;
        self.smtp_sink.kill().await?;

        Ok(TestCase {
            postfix: self.postfix,
            smtp_sink: self.smtp_sink,
            state: PhantomData,
        })
    }
}

impl TestCase<StoppedState> {
    pub async fn log_file_content(&self) -> Result<String> {
        self.postfix.log_file_content().await
    }

    pub async fn validate_mail(&self, needle: &str) -> Result<String> {
        let changed_file = self
            .smtp_sink
            .wait_for_file()
            .await
            .wrap_err("Failed watching files")?;

        let content = fs::read_to_string(changed_file.as_path())
            .await
            .into_diagnostic()
            .wrap_err("Could not read mail output file")?;

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
