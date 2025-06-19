use crate::utils::TestCase;
use async_trait::async_trait;
use miette::Error as ErrReport;
use miltr_common::{
    actions::{Action, Continue},
    modifications::{body::ReplaceBody, ModificationResponse},
};
use miltr_server::Milter;

#[derive(Debug, Clone)]
struct ReplaceBodyTestMilter;

#[async_trait]
impl Milter for ReplaceBodyTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();
        builder.push(ReplaceBody::new("Replace Body\r\n".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}

#[tokio::test]
async fn test_replace_body() {
    let testcase = TestCase::setup("modifications-body-replace-body", ReplaceBodyTestMilter)
        .await
        .expect("Failed setting up test case");

    testcase.send_mail().await.expect("Failed sending mail");
    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    testcase
        .validate_mail("Replace Body")
        .await
        .expect("Received mail did not contain replaced body");
}
