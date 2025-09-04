//! A milter that prints callback arguments and macros for each stage.

use async_trait::async_trait;
use std::env;
use tokio::net::TcpListener;
use tokio_util::compat::TokioAsyncReadCompatExt;

use miltr_common::{
    actions::{Action, Continue, Replycode},
    commands::{Body, Header},
    modifications::{body::ReplaceBody, headers::ChangeHeader, ModificationResponse},
};
use miltr_server::{Milter, Server};

#[derive(Debug, Default)]
struct ModMilter {
    headers: Vec<Header>,
    body_parts: Vec<Body>,
}

#[async_trait]
impl Milter for ModMilter {
    type Error = &'static str;

    async fn header(&mut self, header: Header) -> Result<Action, Self::Error> {
        self.headers.push(header);
        Ok(Continue.into())
    }

    async fn body(&mut self, body: Body) -> Result<Action, Self::Error> {
        self.body_parts.push(body);
        Ok(Continue.into())
    }

    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        let mut builder = ModificationResponse::builder();

        if let Some(last_header) = self.headers.last() {
            let new_value = format!("{} {}", last_header.value(), "was changed");
            builder.push(ChangeHeader::new(
                u32::try_from(self.headers.len())
                    .map_err(|_e| "Failed converting header length")?,
                last_header.name().as_bytes(),
                new_value.as_bytes(),
            ));
        }

        for body_part in &self.body_parts {
            let upper = String::from_utf8_lossy(body_part.as_bytes()).to_uppercase();
            builder.push(ReplaceBody::from_iter(upper.as_bytes()));
        }

        Ok(builder.build(Replycode::new([1, 2, 3], [4, 5, 6], "What a message!")))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        println!("\n======== ABORT ========");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let addr = env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to addr");
    println!("Bound to socket");

    let mut milter = ModMilter::default();
    let mut server = Server::default_postfix(&mut milter);

    loop {
        println!("Accepting connections");
        let (stream, _socket_addr) = listener
            .accept()
            .await
            .expect("Failed accepting connection");
        server
            .handle_connection(&mut stream.compat())
            .await
            .expect("Failed handling this connection");
    }
}
