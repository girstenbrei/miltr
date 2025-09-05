use crate::utils::TestCase;
use async_trait::async_trait;
use miette::{Error as ErrReport, Result};
use miltr_common::{
    commands::Macro,
    optneg::{MacroStage, OptNeg},
};
use miltr_server::{Error, Milter};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
struct MacroRequestTestMilter {
    sender: mpsc::Sender<Macro>,
}

impl MacroRequestTestMilter {
    pub fn new(sender: mpsc::Sender<Macro>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl Milter for MacroRequestTestMilter {
    type Error = ErrReport;
    async fn option_negotiation(&mut self, _: OptNeg) -> Result<OptNeg, Error<Self::Error>> {
        let mut optneg = OptNeg::default();
        optneg
            .macro_stages
            .with_stage(MacroStage::Connect, &["j", "{daemon_addr}"]);
        optneg.macro_stages.with_stage(MacroStage::Helo, &["z"]);
        optneg.macro_stages.with_stage(MacroStage::MailFrom, &["z"]);
        optneg.macro_stages.with_stage(MacroStage::RcptTo, &["z"]);
        optneg.macro_stages.with_stage(MacroStage::Data, &["z"]);
        optneg.macro_stages.with_stage(MacroStage::Header, &["z"]);
        optneg
            .macro_stages
            .with_stage(MacroStage::EndOfHeaders, &["z"]);
        optneg.macro_stages.with_stage(MacroStage::Body, &["z"]);
        optneg
            .macro_stages
            .with_stage(MacroStage::EndOfBody, &["{daemon_addr}"]);

        Ok(optneg)
    }

    async fn macro_(&mut self, macr: Macro) -> Result<()> {
        self.sender.send(macr).await.expect("Failed sending macro");
        Ok(())
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

const RECV_MACROS_MAX: usize = 17;

/// Test Macro Request.
/// Test example:
/// Default macros for Connect `MacroStage` : "`j","{client_addr}","{client_connections`}", "{`client_name`}", "{`client_port`}", "{`client_ptr`}", "{`daemon_addr`}", "{`daemon_name`}", "{`daemon_port`}", "v" .
/// But we will only send "`j","{client_addr}","{client_connections`}" in Connect `MacroStage` (more details in optneg.rs) .
/// If Milter and Postfix work, we will receive:
///Macro { code: b'C', body: `b"j\x00localhost\x00{client_addr}\x00127.0.0.1\x00{client_connections}\x000\x00`}
#[tokio::test]
async fn test_macro_request() {
    let (tx, mut rx) = mpsc::channel(RECV_MACROS_MAX);
    let milter = MacroRequestTestMilter::new(tx);

    let testcase = TestCase::setup("modifications-macro", milter)
        .await
        .expect("Failed setting up test case");

    testcase.send_mail().await.expect("Failed sending mail");
    let _testcase = testcase.stop().await.expect("Failed to shut down postfix");

    let mut macros = Vec::with_capacity(RECV_MACROS_MAX);
    rx.recv_many(&mut macros, RECV_MACROS_MAX).await;

    assert_eq!(macros[0].code, b'C');
    let expected: Vec<(&[u8], &[u8])> =
        vec![(b"j", b"localhost"), (b"{daemon_addr}", b"127.0.0.1")];
    for ((key, value), (ekey, evalue)) in macros[0].macros().zip(expected) {
        assert_eq!(key, ekey);
        assert_eq!(value, evalue);
    }

    assert_eq!(macros[1].code, b'H');
    assert_eq!(macros[1].macros().count(), 0);

    assert_eq!(macros[macros.len() - 1].code, b'E');
    let expected: Vec<(&[u8], &[u8])> = vec![(b"{daemon_addr}", b"127.0.0.1")];
    for ((key, value), (ekey, evalue)) in macros[macros.len() - 1].macros().zip(expected) {
        assert_eq!(key, ekey);
        assert_eq!(value, evalue);
    }
}
