mod action_milter;
mod milter_mock;
mod portguard;
mod postfix;
mod testcase;

use miette::miette;
use std::fmt::{Debug, Display};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::TokioAsyncReadCompatExt;

pub use action_milter::ActionMilter;
pub use milter_mock::MilterMockBuilder;
pub use testcase::TestCase;

use miltr_server::{Milter, Server};
use portguard::PortGuard;
use postfix::PostfixInstance;

pub async fn run_milter<M, E>(listener: TcpListener, milter: M)
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
            tokio::spawn(async move { handle_connection(stream, inner_milter).await });
        }
    });
}

async fn handle_connection<M, E>(stream: TcpStream, mut milter: M) -> miette::Result<()>
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
