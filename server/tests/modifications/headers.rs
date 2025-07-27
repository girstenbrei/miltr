use crate::utils::TestCase;
use async_trait::async_trait;
use miette::Error as ErrReport;
use miltr_common::{
    actions::{Action, Continue},
    modifications::{
        headers::{AddHeader, ChangeHeader, InsertHeader},
        ModificationResponse,
    },
};
use miltr_server::Milter;

#[derive(Debug, Default, Clone)]
struct AddHeaderMilter;

#[async_trait]
impl Milter for AddHeaderMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(AddHeader::new(
            "Test Add Header".as_bytes(),
            "Add Header Value".as_bytes(),
        ));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}

#[tokio::test]
async fn test_add_header() {
    let testcase = TestCase::setup("modifications-headers-add-header", AddHeaderMilter)
        .await
        .expect("Failed setting up test case");

    let response = testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    testcase
        .validate_mail("Test Add Header: Add Header Value", &response)
        .await
        .expect("Did not find added header in received mail");
}

#[derive(Debug, Default, Clone)]
struct ChangeHeaderMilter;

#[async_trait]
impl Milter for ChangeHeaderMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(ChangeHeader::new(
            1,
            "Subject".as_bytes(),
            "Change Header Value".as_bytes(),
        ));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test]
async fn test_change_header() {
    let testcase = TestCase::setup("modifications-headers-change-header", ChangeHeaderMilter)
        .await
        .expect("Failed setting up test case");

    let response = testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    testcase
        .validate_mail("Subject: Change Header Value", &response)
        .await
        .expect("Did not find changed header in received mail");
}

#[derive(Debug, Clone)]
struct InsertHeaderMilter;

#[async_trait]
impl Milter for InsertHeaderMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(InsertHeader::new(
            1,
            "Insert Header".as_bytes(),
            "Insert Header Value".as_bytes(),
        ));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test]
async fn test_insert_header() {
    let testcase = TestCase::setup("modifications-headers-insert-header", InsertHeaderMilter)
        .await
        .expect("Failed setting up test case");

    let response = testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    testcase
        .validate_mail("Insert Header: Insert Header Value", &response)
        .await
        .expect("DId not find inserted header in received mail");
}
