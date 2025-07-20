use crate::utils::TestCase;
use async_trait::async_trait;
use miette::Error as ErrReport;
use miltr_common::{
    actions::{Action, Continue},
    modifications::{
        recipients::{AddRecipient, DeleteRecipient},
        ModificationResponse,
    },
};
use miltr_server::Milter;

///This does not change To in Header
#[derive(Debug, Clone)]
struct AddRcptTestMilter;

#[async_trait]
impl Milter for AddRcptTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(AddRecipient::new(
            "<add_rcpt-added@blackhole.com>".as_bytes(),
        ));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test]
async fn test_add_rcpt() {
    let testcase = TestCase::setup("modifications-recipient-add-rcpt", AddRcptTestMilter)
        .await
        .expect("Failed setting up test case");

    let response = testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    testcase
        .validate_mail("recipient: add_rcpt-added@blackhole.com", &response)
        .await
        .expect("Received mail did not contain added recipient");
}

///This doesn not change To in Header
#[derive(Debug, Clone)]
struct DeleteRcptTestMilter;

#[async_trait]
impl Milter for DeleteRcptTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(DeleteRecipient::new("Hei <hei@domain.tld>".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test]
async fn test_delete_rcpt() {
    let testcase = TestCase::setup("modifications-recipient-delete-rcpt", DeleteRcptTestMilter)
        .await
        .expect("Failed setting up test case");

    let response = testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Dont send mail to test.local-1@blackhole.com -> X-Rcpt-Args: <test.local-1@blackhole.com> will not be found -> validate_mail will return Error
    testcase
        .validate_mail("X-Rcpt-Args: <delete_rcpt@blackhole.com>", &response)
        .await
        .expect_err("Deleting the recipient did not delete the mails");
}
