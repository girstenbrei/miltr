//! Test utility to simulate any milter handler.
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use miette::Result;

use miltr_common::actions::{Action, Continue};
use miltr_common::commands::{Body, Connect, Header, Helo, Macro, Mail, Recipient, Unknown};
use miltr_common::modifications::ModificationResponse;
use miltr_common::optneg::OptNeg;
use miltr_server::Error;
use miltr_server::Milter;

type HandlerActionResult = Pin<Box<dyn Future<Output = Action> + Send>>;
type HandlerEmptyResult = Pin<Box<dyn Future<Output = ()> + Send>>;

type OnOptNeg =
    Option<Box<dyn Fn(OptNeg) -> Pin<Box<dyn Future<Output = OptNeg> + Send>> + Send + Sync>>;
type OnMacro = Option<Box<dyn Fn(Macro) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>>;
type OnConnect = Option<Box<dyn Fn(Connect) -> HandlerActionResult + Send + Sync>>;
type OnHelo = Option<Box<dyn Fn(Helo) -> HandlerActionResult + Send + Sync>>;
type OnMail = Option<Box<dyn Fn(Mail) -> HandlerActionResult + Send + Sync>>;
type OnRcpt = Option<Box<dyn Fn(Recipient) -> HandlerActionResult + Send + Sync>>;
type OnData = Option<Box<dyn Fn() -> HandlerActionResult + Send + Sync>>;
type OnHeader = Option<Box<dyn Fn(Header) -> HandlerActionResult + Send + Sync>>;
type OnEndOfHeader = Option<Box<dyn Fn() -> HandlerActionResult + Send + Sync>>;
type OnBody = Option<Box<dyn Fn(Body) -> HandlerActionResult + Send + Sync>>;
type OnEndOfBody = Option<
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = ModificationResponse> + Send>> + Send + Sync>,
>;
type OnUnknown = Option<Box<dyn Fn(Unknown) -> HandlerActionResult + Send + Sync>>;
type OnAbort = Option<Box<dyn Fn() -> HandlerEmptyResult + Send + Sync>>;
type OnQuit = Option<Box<dyn Fn() -> HandlerEmptyResult + Send + Sync>>;
type OnQuitNc = Option<Box<dyn Fn() -> HandlerEmptyResult + Send + Sync>>;

#[derive(Default)]
struct Handlers {
    opt_neg: OnOptNeg,
    r#macro: OnMacro,
    connect: OnConnect,
    helo: OnHelo,
    mail: OnMail,
    rcpt: OnRcpt,
    data: OnData,
    header: OnHeader,
    end_of_header: OnEndOfHeader,
    body: OnBody,
    end_of_body: OnEndOfBody,
    unknown: OnUnknown,
    abort: OnAbort,
    quit: OnQuit,
    quit_nc: OnQuitNc,
}

pub struct MilterMockBuilder {
    handlers: Handlers,
    default_action: Action,
}

#[allow(dead_code)]
impl MilterMockBuilder {
    pub fn new() -> Self {
        Self {
            handlers: Handlers::default(),
            default_action: Continue.into(),
        }
    }

    pub fn with_opt_neg_handler(
        mut self,
        handler: impl Fn(OptNeg) -> Pin<Box<dyn Future<Output = OptNeg> + Send>> + Send + Sync + 'static,
    ) -> Self {
        self.handlers.opt_neg = Some(Box::new(handler));
        self
    }

    pub fn with_macro_handler(
        mut self,
        handler: impl Fn(Macro) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    ) -> Self {
        self.handlers.r#macro = Some(Box::new(handler));
        self
    }

    pub fn with_connect_handler(
        mut self,
        handler: impl Fn(Connect) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.connect = Some(Box::new(handler));
        self
    }

    pub fn with_helo_handler(
        mut self,
        handler: impl Fn(Helo) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.helo = Some(Box::new(handler));
        self
    }

    pub fn with_mail_handler(
        mut self,
        handler: impl Fn(Mail) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.mail = Some(Box::new(handler));
        self
    }

    pub fn with_rcpt_handler(
        mut self,
        handler: impl Fn(Recipient) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.rcpt = Some(Box::new(handler));
        self
    }

    pub fn with_data_handler(
        mut self,
        handler: impl Fn() -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.data = Some(Box::new(handler));
        self
    }

    pub fn with_header_handler(
        mut self,
        handler: impl Fn(Header) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.header = Some(Box::new(handler));
        self
    }

    pub fn with_end_of_header_handler(
        mut self,
        handler: impl Fn() -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.end_of_header = Some(Box::new(handler));
        self
    }

    pub fn with_body_handler(
        mut self,
        handler: impl Fn(Body) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.body = Some(Box::new(handler));
        self
    }

    pub fn with_end_of_body_handler(
        mut self,
        handler: impl Fn() -> Pin<Box<dyn Future<Output = ModificationResponse> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.handlers.end_of_body = Some(Box::new(handler));
        self
    }

    pub fn with_unknown_handler(
        mut self,
        handler: impl Fn(Unknown) -> HandlerActionResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.unknown = Some(Box::new(handler));
        self
    }

    pub fn with_abort_handler(
        mut self,
        handler: impl Fn() -> HandlerEmptyResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.abort = Some(Box::new(handler));
        self
    }

    pub fn with_quit_handler(
        mut self,
        handler: impl Fn() -> HandlerEmptyResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.quit = Some(Box::new(handler));
        self
    }

    pub fn with_quit_nc_handler(
        mut self,
        handler: impl Fn() -> HandlerEmptyResult + Send + Sync + 'static,
    ) -> Self {
        self.handlers.quit_nc = Some(Box::new(handler));
        self
    }

    pub fn with_default_action(mut self, action: impl Into<Action>) -> Self {
        self.default_action = action.into();
        self
    }

    pub fn build(self) -> MilterMock {
        MilterMock {
            inner: Arc::new(self.handlers),
            default_action: self.default_action,
            end_of_body_called: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[derive(Clone)]
pub struct MilterMock {
    inner: Arc<Handlers>,
    default_action: Action,
    end_of_body_called: Arc<AtomicUsize>,
}

impl MilterMock {
    pub fn end_of_body_called(&self) -> usize {
        self.end_of_body_called.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Milter for MilterMock {
    type Error = &'static str;

    async fn option_negotiation(&mut self, theirs: OptNeg) -> Result<OptNeg, Error<Self::Error>> {
        if let Some(handler) = &self.inner.opt_neg {
            Ok(handler(theirs).await)
        } else {
            let mut ours = OptNeg::default();
            ours = ours
                .merge_compatible(&theirs)
                .expect("Can't merge compatible opt-neg values");
            Ok(ours)
        }
    }

    async fn macro_(&mut self, macro_: Macro) -> Result<(), Self::Error> {
        if let Some(handler) = &self.inner.r#macro {
            handler(macro_).await;
        }
        Ok(())
    }

    async fn connect(&mut self, connect_info: Connect) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.connect {
            Ok(handler(connect_info).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn helo(&mut self, helo: Helo) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.helo {
            Ok(handler(helo).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn mail(&mut self, mail: Mail) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.mail {
            Ok(handler(mail).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn rcpt(&mut self, recipient: Recipient) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.rcpt {
            Ok(handler(recipient).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn data(&mut self) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.data {
            Ok(handler().await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn header(&mut self, header: Header) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.header {
            Ok(handler(header).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn end_of_header(&mut self) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.end_of_header {
            Ok(handler().await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn body(&mut self, body: Body) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.body {
            Ok(handler(body).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        self.end_of_body_called.fetch_add(1, Ordering::SeqCst);
        if let Some(handler) = &self.inner.end_of_body {
            Ok(handler().await)
        } else {
            Ok(ModificationResponse::empty_continue())
        }
    }

    async fn unknown(&mut self, cmd: Unknown) -> Result<Action, Self::Error> {
        if let Some(handler) = &self.inner.unknown {
            Ok(handler(cmd).await)
        } else {
            Ok(self.default_action.clone())
        }
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        if let Some(handler) = &self.inner.abort {
            handler().await;
        }
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), Self::Error> {
        if let Some(handler) = &self.inner.quit {
            handler().await;
        }
        Ok(())
    }

    async fn quit_nc(&mut self) -> Result<(), Self::Error> {
        if let Some(handler) = &self.inner.quit_nc {
            handler().await;
        }
        Ok(())
    }
}
