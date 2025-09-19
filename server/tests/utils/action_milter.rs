//! Test utils to run a single action during a milter session.
use async_trait::async_trait;
use miette::{miette, Result};
use std::{
    fmt::{Debug, Display},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::TokioAsyncReadCompatExt;

use miltr_common::{actions::Action, modifications::ModificationResponse};
use miltr_server::{Milter, Server};

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
            action_called: AtomicUsize::default(),
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

pub async fn run_milter<M, E>(listener: TcpListener, milter: M, connect_count: Arc<AtomicUsize>)
where
    E: Debug + Display + 'static,
    M: Milter<Error = E> + 'static + Clone,
{
    tokio::spawn(async move {
        loop {
            println!(
                "Accepting miltr connections on {}",
                &listener
                    .local_addr()
                    .expect("Failed getting local addr")
                    .port()
            );
            let Ok((stream, _socket_addr)) = listener.accept().await else {
                println!("Accept not ok");
                continue;
            };
            let inner_milter = milter.clone();
            connect_count.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move { handle_connection(stream, inner_milter).await });
        }
    });
}

pub async fn handle_connection<M, E>(stream: TcpStream, mut milter: M) -> Result<()>
where
    E: Debug + Display,
    M: Milter<Error = E> + 'static,
{
    let mut server = Server::default_postfix(&mut milter);
    server
        .handle_connection(&mut stream.compat())
        .await
        .map_err(|e| miette!("{e}"))
}
