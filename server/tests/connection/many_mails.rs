use std::sync::atomic::AtomicUsize;
use std::sync::{atomic, Arc};
use std::time::Duration;

use async_trait::async_trait;
use miette::{ErrReport, Result};
use tokio::time::sleep;

use miltr_common::actions::Action;
use miltr_common::actions::Continue;
use miltr_common::commands::Mail;
use miltr_common::modifications::headers::AddHeader;
use miltr_common::modifications::ModificationResponse;
use miltr_server::Milter;

use crate::utils::TestCase;

const MAILS_COUNT: usize = 5;
const MAIL_COMMAND_SLEEP_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
struct AddHeaderTestMilter {
    end_of_body_called: Arc<AtomicUsize>,
}

impl AddHeaderTestMilter {
    pub fn new() -> Self {
        Self {
            end_of_body_called: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn end_of_body_called(&self) -> usize {
        self.end_of_body_called
            .as_ref()
            .load(atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl Milter for AddHeaderTestMilter {
    type Error = ErrReport;

    async fn mail(&mut self, _mail: Mail) -> std::result::Result<Action, Self::Error> {
        // This wait is necessary to reproduce the milter protocol violation issues
        // with the postfix state machine.
        // It seems that if `Mail` handler response is delayed - postfix starts processing
        // the response of the previous EOB, and throws an error.
        sleep(MAIL_COMMAND_SLEEP_DURATION).await;
        Ok(Continue.into())
    }

    async fn end_of_body(&mut self) -> std::result::Result<ModificationResponse, Self::Error> {
        self.end_of_body_called
            .fetch_add(1, atomic::Ordering::SeqCst);
        let mut builder = ModificationResponse::builder();
        builder.push(AddHeader::new(b"name", b"value"));
        builder.push(AddHeader::new(b"name1", b"value1"));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn many_mail_send_test() -> Result<()> {
    let milter = AddHeaderTestMilter::new();
    let testcase = TestCase::setup("many-mail-send", milter.clone()).await?;
    let mut transport = testcase.create_smtp_transport().await;
    for i in 0..MAILS_COUNT {
        if transport.has_broken() {
            break;
        }
        println!("Sending email num {}", i + 1);
        let _ = testcase
            .send_mail_with_transport(&mut transport)
            .await
            // Sending errors are ignored.
            // This is done to check their cause later in the postifx logs.
            .map_err(|e| println!("Error on send mails: {e}"));
    }
    transport.abort().await;
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file for expected content
    let content = testcase.log_file_content().await?;
    assert!(!content.contains("unexpected filter response"));

    // Check that the milter was actually called
    assert_eq!(milter.end_of_body_called(), MAILS_COUNT);
    // Check that the miller worked with a single connection
    assert_eq!(testcase.milter_connections_count(), 1);
    Ok(())
}
