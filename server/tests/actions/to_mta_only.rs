use miette::Result;
use miltr_common::actions::{Discard, Reject, Replycode, Skip, Tempfail};

use crate::utils::{ActionMilter, TestCase};

#[tokio::test]
async fn discard() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Discard);

    let testcase = TestCase::setup("actions-to-mta-only-discard", milter.clone()).await?;
    testcase
        .send_mail()
        .await
        .expect("Mail should be processed normally");

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file for
    let content = testcase.log_file_content().await?;

    assert!(content.contains("message-id="));
    assert!(content.contains("milter triggers DISCARD"));

    assert_eq!(milter.action_called(), 1);

    Ok(())
}

#[tokio::test]
async fn reject() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Reject);

    let testcase = TestCase::setup("actions-to-mta-only-reject", milter.clone()).await?;
    let err = testcase
        .send_mail()
        .await
        .expect_err("Mail should be rejected");
    println!("{err:?}");
    assert!(err.is_permanent());
    assert_eq!(
        err.status().expect("Missing smtp status code").to_string(),
        "550"
    );

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file
    let content = testcase.log_file_content().await?;
    assert!(content.contains("message-id="));
    assert!(content.contains("5.7.1 Command rejected"));

    // And ensure milter was called
    assert!(milter.action_called() >= 1);

    Ok(())
}

#[tokio::test]
async fn tempfail() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Tempfail);

    let testcase = TestCase::setup("actions-to-mta-only-tempfail", milter.clone()).await?;
    let err = testcase
        .send_mail()
        .await
        .expect_err("Mail should be rejected");
    println!("{err:?}");
    assert!(err.is_transient());
    assert_eq!(
        err.status().expect("Missing smtp status code").to_string(),
        "451"
    );

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file
    let content = testcase.log_file_content().await?;
    assert!(content.contains("message-id="));
    assert!(content.contains(
        "milter-reject: END-OF-MESSAGE from unknown[127.0.0.1]: 4.7.1 Service unavailable"
    ));

    // And ensure milter was called
    assert!(milter.action_called() >= 1);

    Ok(())
}

#[tokio::test]
async fn skip() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Skip);

    let testcase = TestCase::setup("actions-to-mta-only-skip", milter.clone()).await?;
    testcase
        .send_mail()
        .await
        .expect("Mail should processed normally");

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file
    let content = testcase.log_file_content().await?;
    assert!(content.contains("message-id="));

    // And ensure milter was called
    assert_eq!(milter.action_called(), 1);

    Ok(())
}

#[tokio::test]
async fn replycode() -> Result<()> {
    // Start the milter
    let milter = ActionMilter::new(Replycode::new([5, 2, 1], [5, 5, 4], "Foobar"));

    let testcase = TestCase::setup("actions-to-mta-only-replycode", milter.clone()).await?;
    let res = testcase
        .send_mail()
        .await
        .expect_err("Mail should be rejected");

    assert_eq!(
        res.status().expect("Missing returned status").to_string(),
        "521"
    );

    let testcase = testcase.stop().await.expect("Failed to shut down postfix");

    // Check the log file
    let content = testcase.log_file_content().await?;
    assert!(content.contains("message-id="));
    assert!(content.contains("milter-reject"));

    // And ensure milter was called
    assert!(milter.action_called() >= 1);

    Ok(())
}
