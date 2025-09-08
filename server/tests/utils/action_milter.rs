//! Test utils to run a single action during a milter session.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use miette::Result;
use miltr_common::{actions::Action, modifications::ModificationResponse};
use miltr_server::Milter;

/// A milter performing a single action on `end_of_body`.
#[derive(Clone)]
pub struct ActionMilter(Arc<ActionMilterInner>);

struct ActionMilterInner {
    action: Action,
    action_called: AtomicUsize,
}

impl ActionMilter {
    pub fn new(action: impl Into<Action>) -> Self {
        Self(Arc::new(ActionMilterInner {
            action: action.into(),
            action_called: AtomicUsize::new(0),
        }))
    }

    pub fn action_called(&self) -> usize {
        self.0.action_called.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Milter for ActionMilter {
    type Error = &'static str;

    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        println!("Body called");
        self.0.action_called.fetch_add(1, Ordering::SeqCst);
        Ok(ModificationResponse::builder().build(self.0.action.clone()))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
