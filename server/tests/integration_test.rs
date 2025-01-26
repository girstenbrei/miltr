//! A module containing different milters to test implementation.
use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use miette::{miette, Context, ErrReport, Result};
use miltr_common::{
    actions::{Action, Continue},
    commands::Macro,
    modifications::{
        body::ReplaceBody,
        headers::{AddHeader, ChangeHeader, InsertHeader},
        quarantine::Quarantine,
        recipients::{AddRecipient, DeleteRecipient},
        ModificationResponse,
    },
    optneg::{MacroStage, OptNeg},
};
use miltr_server::{Error, Milter};
use once_cell::sync::Lazy;
use tokio::{
    process::Command,
    sync::mpsc::{self, Sender},
};
use tokio_retry::{
    strategy::{jitter, FixedInterval},
    Retry,
};
use utils::wait_for_file;

use crate::utils::{remove_dir_contents, send_mail, testcase::TestCase};

mod utils;

static BASE_PATH: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("./emails"));

async fn validate_mail(needle: &str, path: &Path) -> Result<String> {
    let changed_file = wait_for_file(path)
        .await
        .wrap_err("Failed watching files")?;

    let content =
        fs::read_to_string(changed_file.as_path()).expect("Should have been able to read the file");

    if content.contains(needle) {
        Ok(content)
    } else {
        Err(miette!("Email is not correctly modified by Milter"))
    }
}

#[derive(Debug, Default, Clone)]
struct AddHeaderTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for AddHeaderTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
        //For example: milter performs ADDHEADER
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

#[tokio::test(flavor = "multi_thread")]
async fn test_add_header() {
    let test_name = "add_header";
    let path = BASE_PATH.clone().join(test_name);

    let milter: AddHeaderTestMilter = AddHeaderTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("test.local-2@blackhole.com")
        .await
        .expect("Failed sending mail");

    validate_mail("Test Add Header: Add Header Value", &path)
        .await
        .expect("Can not add header");
}

#[derive(Debug, Default, Clone)]
struct ChangeHeaderTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for ChangeHeaderTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
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
#[tokio::test(flavor = "multi_thread")]
async fn test_change_header() {
    let test_name = "change_header";
    let path = BASE_PATH.clone().join(test_name);

    let milter: ChangeHeaderTestMilter = ChangeHeaderTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("test.local-2@blackhole.com")
        .await
        .expect("Failed sending mail");

    validate_mail("Subject: Change Header Value", &path)
        .await
        .expect("Can not change header");
}

#[derive(Debug, Clone)]
struct InsertHeaderTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for InsertHeaderTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
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
#[tokio::test(flavor = "multi_thread")]
async fn test_insert_header() {
    let test_name = "insert_header";
    let path = BASE_PATH.clone().join(test_name);

    let milter = InsertHeaderTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("test.local-2@blackhole.com")
        .await
        .expect("Failed sending mail");

    validate_mail("Insert Header: Insert Header Value", &path)
        .await
        .expect("Can not insert header");
}

#[derive(Debug, Clone)]
struct ReplaceBodyTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for ReplaceBodyTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
        let mut builder = ModificationResponse::builder();
        builder.push(ReplaceBody::new("Replace Body\r\n".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_replace_body() {
    let test_name = "replace_body";
    let path = BASE_PATH.clone().join(test_name);

    let milter = ReplaceBodyTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("test.local-2@blackhole.com")
        .await
        .expect("Failed sending mail");

    validate_mail("Replace Body", &path)
        .await
        .expect("Can not replace header");
}

///This does not change To in Header
#[derive(Debug, Clone)]
struct AddRcptTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for AddRcptTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
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
#[tokio::test(flavor = "multi_thread")]
async fn test_add_rcpt() {
    let test_name = "add_rcpt";
    let path = BASE_PATH.clone().join(test_name);

    let milter = AddRcptTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("add_rcpt@blackhole.com")
        .await
        .expect("Failed sending mail");

    validate_mail("X-Rcpt-Args: <add_rcpt-added@blackhole.com>", &path)
        .await
        .expect("Can not add rcpt");
}

///This doesn not change To in Header
#[derive(Debug, Clone)]
struct DeleteRcptTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for DeleteRcptTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
        let mut builder = ModificationResponse::builder();
        builder.push(DeleteRecipient::new("delete_rcpt@blackhole.com".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn test_delete_rcpt() {
    let test_name = "delete_rcpt";
    let path = BASE_PATH.clone().join(test_name);

    let milter = DeleteRcptTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("delete_rcpt@blackhole.com")
        .await
        .expect("Failed sending mail");

    //Dont send mail to test.local-1@blackhole.com -> X-Rcpt-Args: <test.local-1@blackhole.com> will not be found -> validate_mail will return Error
    validate_mail("X-Rcpt-Args: <delete_rcpt@blackhole.com>", &path)
        .await
        .expect_err("Deleting the recipient did not delete the mails");
}

/// This quarantines the message into a holding pool (/var/spool/postfix/hold) defined by the MTA.
/// (First implemented in Sendmail in version 8.13; offered to the milter by
///    the `SMFIF_QUARANTINE` flag in "actions" of `SMFIC_OPTNEG`.)
#[derive(Debug, Clone)]
struct QuarantineTestMilter {
    commands: Vec<String>,
}

#[async_trait]
impl Milter for QuarantineTestMilter {
    type Error = ErrReport;
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.commands.push("end_of_body".to_string());
        let mut builder = ModificationResponse::builder();
        builder.push(Quarantine::new("Invalid Email".as_bytes()));
        let response = builder.contin();
        Ok(response)
    }

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn test_quarantine() {
    let test_name = "quarantine";
    let path = BASE_PATH.clone().join(test_name);

    let spool_dir = PathBuf::from("/var/spool/postfix/hold");

    if spool_dir.exists() {
        remove_dir_contents(&spool_dir)
            .await
            .expect("Failed to empty holding spool");
    }

    let milter = QuarantineTestMilter {
        commands: Vec::new(),
    };

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("test.local-2@blackhole.com")
        .await
        .expect("Failed sending mail");

    let retry_strategy = FixedInterval::from_millis(1000).map(jitter).take(10);
    let result = Retry::spawn(retry_strategy, || {
        async move {
            //Return the amount of file in holding pool -> after using quarantie wc -l should return 1
            let open_mail = Command::new("sh")
                .arg("-c")
                .arg("cd /var/spool/postfix/hold && ls | wc -l")
                .output()
                .await
                .unwrap();

            let res_mail = String::from_utf8_lossy(&open_mail.stdout);

            let content = res_mail.into_owned();

            if content == *"1\n" {
                Ok(content)
            } else {
                Err("Can not send quarantine mail to holding spool ")
            }
        }
    })
    .await;

    result.expect("Can not quarantine");
}

#[derive(Debug)]
struct MacroRequestTestMilter {
    sender: Sender<Macro>,
}

impl MacroRequestTestMilter {
    pub fn new(sender: Sender<Macro>) -> Self {
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

    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}

/// Test Macro Request.
/// Test example:
/// Default macros for Connect MacroStage : "j","{client_addr}","{client_connections}", "{client_name}", "{client_port}", "{client_ptr}", "{daemon_addr}", "{daemon_name}", "{daemon_port}", "v" .
/// But we will only send "j","{client_addr}","{client_connections}" in Connect MacroStage (more details in optneg.rs) .
/// If Milter and Postfix work, we will receive:
///Macro { code: b'C', body: b"j\x00localhost\x00{client_addr}\x00127.0.0.1\x00{client_connections}\x000\x00}
#[tokio::test(flavor = "multi_thread")]
async fn test_macro_request() {
    let test_name = "macro_request";
    let path = BASE_PATH.clone().join(test_name);

    let (tx, mut rx) = mpsc::channel(14);

    let milter = MacroRequestTestMilter::new(tx);

    let _testcase_guard = TestCase::setup(milter, &path)
        .await
        .expect("Failed setting up test case");

    send_mail("macro_request@blackhole.com")
        .await
        .expect("Failed sending mail");

    let mut macros = Vec::new();
    rx.recv_many(&mut macros, 14).await;

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
