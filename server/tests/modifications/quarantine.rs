use crate::utils::TestCase;
use async_trait::async_trait;
use miette::Error as ErrReport;
use miltr_common::modifications::{quarantine::Quarantine, ModificationResponse};
use miltr_server::Milter;

/// This quarantines the message into a holding pool (/var/spool/postfix/hold) defined by the MTA.
/// (First implemented in Sendmail in version 8.13; offered to the milter by
///    the `SMFIF_QUARANTINE` flag in "actions" of `SMFIC_OPTNEG`.)
#[derive(Debug, Clone)]
struct QuarantineTestMilter;

#[async_trait]
impl Milter for QuarantineTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(Quarantine::new("Invalid Email".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[tokio::test]
async fn test_quarantine() {
    let testcase = TestCase::setup("modifications-quarantine", QuarantineTestMilter)
        .await
        .expect("Failed setting up test case");

    testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    let content = testcase
        .log_file_content()
        .await
        .expect("Failed reading logfile");
    assert!(content.contains("milter triggers HOLD action"));
}
