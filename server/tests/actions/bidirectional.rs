use miette::Result;
use miltr_common::actions::{Abort, Continue};

use crate::utils::{ActionMilter, TestCase};

#[tokio::test]
async fn abort() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Abort);

    let testcase = TestCase::setup("actions-bidirectional-abort", milter.clone()).await?;

    testcase
        .send_mail()
        .await
        .expect_err("We except an abort to be called here");

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file for expected content
    let content = testcase.log_file_content().await?;
    assert!(content
        .contains("unexpected filter response (unknown filter reply) after event SMFIC_BODY"));

    // Check that the milter was actually called as expected at least once.
    // The send is retried multiple times and for this abort test,
    // we don't know if this is due to the milter not reachable yet or the
    // returned abort.
    assert!(milter.action_called() >= 1);

    Ok(())
}

#[tokio::test]
async fn r#continue() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Continue);

    let testcase = TestCase::setup("actions-bidirectional-continue", milter.clone()).await?;
    testcase
        .send_mail()
        .await
        .expect("Mail should be processed normally");

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file for
    let content = testcase.log_file_content().await?;

    assert!(content.contains("message-id="));
    assert!(content.contains("from=<nobody@domain.tld>"));
    assert!(content.contains("(queue active)"));

    assert_eq!(milter.action_called(), 1);

    Ok(())
}
