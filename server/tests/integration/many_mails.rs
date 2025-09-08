use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

use lettre::address::Envelope;
use lettre::Address;
use miette::Result;

use miltr_common::actions::Continue;
use miltr_common::modifications::headers::InsertHeader;
use miltr_common::modifications::ModificationResponse;

use crate::utils::MilterMockBuilder;

use crate::utils::TestCase;

#[tokio::test]
async fn many_mail_send_test() -> Result<()> {
    let milter = MilterMockBuilder::new()
        // We make a small delay to simulate the command processing
        .with_mail_handler(|_mail| {
            Box::pin(async {
                sleep(Duration::from_millis(100)).await;
                Continue.into()
            })
        })
        // Sending a modified header
        .with_end_of_body_handler(|| {
            Box::pin(async {
                println!("End of body called");
                let mut modifications = ModificationResponse::builder();
                modifications.push(InsertHeader::new(0, b"name", b"value"));
                modifications.contin()
            })
        })
        .build();

    let mut testcase = TestCase::setup("many-mail-send", milter.clone()).await?;
    let _ = testcase
        .with_smtp_connection(
            |mut connection| async move {
                let envelope = Envelope::new(
                    Some("nobody@domain.tld".parse::<Address>()?),
                    vec!["hei@domain.tld".parse::<Address>()?],
                )
                .expect("Failed evnelope building");
                let email_body = "Be happy!".as_bytes();
                for i in 1..6 {
                    println!("Sending email num {i}");
                    connection.send(&envelope, email_body).await?;
                    connection.command("RSET\r\n").await?;
                }
                Ok::<(), Box<dyn Error>>(())
            },
            None,
            None,
        )
        .await
        .map_err(|e| println!("Error on send mails: {e}"));

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file for expected content
    let content = testcase.log_file_content().await?;
    assert!(!content.contains("unexpected filter response"));

    // Check that the milter was actually called
    assert_eq!(milter.end_of_body_called(), 5);
    Ok(())
}
