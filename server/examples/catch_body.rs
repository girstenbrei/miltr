//! An example printing the complete milter conversation.
use std::env;

use async_trait::async_trait;
use miette::{IntoDiagnostic, Result, WrapErr};
use miltr_common::{
    actions::{Action, Continue},
    commands::{Body, Recipient},
    optneg::{Capability, OptNeg, Protocol},
};
use miltr_server::{Error, Milter, Server};
use tokio::net::TcpListener;
use tokio_util::compat::TokioAsyncReadCompatExt;

#[derive(Debug, Default)]
struct PrintBodyMilter {
    body_parts: Vec<Body>,
}

#[async_trait]
impl Milter for PrintBodyMilter {
    type Error = &'static str;

    /// Option negotation tells the milter client what information this milter
    /// would like to get.
    async fn option_negotiation(&mut self, _: OptNeg) -> Result<OptNeg, Error<Self::Error>> {
        // In this example, we only need to receive the body.
        // So we let postfix know, we don't want to have all the other info.
        let protocol = Protocol::empty()
            | Protocol::NO_CONNECT
            | Protocol::NO_HELO
            | Protocol::NO_MAIL
            | Protocol::NO_RECIPIENT
            | Protocol::NO_HEADER
            | Protocol::NO_END_OF_HEADER;

        // The default includes all commands and capabilities
        let optneg = OptNeg {
            // But this example actually does not modify anything, it does not
            // have the 'Capabilites' to do so.
            capabilities: Capability::empty(),
            protocol,
            ..Default::default()
        };

        Ok(optneg)
    }

    /// This example errors on the rcpt command: Option negotiation told postfix
    /// to omit this command, this is just to demonstrate you can skip commands.
    async fn rcpt(&mut self, _: Recipient) -> Result<Action, Self::Error> {
        println!("This should not be printed as optneg said SMFIP_NORCPT");

        Err("Got unexpected command")
    }

    /// The body command might be received multiple times, so we push all the
    /// received bodies on a vec.
    async fn body(&mut self, body: Body) -> Result<Action, Self::Error> {
        self.body_parts.push(body);
        Ok(Continue.into())
    }

    /// Receiving an abort denotes the point in time we will have the most
    /// body parts. The client must not send more afterwards.
    async fn abort(&mut self) -> Result<(), Self::Error> {
        println!("\n======== ABORT ========");

        println!("Captured body:");
        println!("--------------");
        for part in &self.body_parts {
            println!("{}", String::from_utf8_lossy(part.as_bytes()));
        }
        println!("--------------");
        println!("End of body");

        //
        self.body_parts.truncate(0);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to addr");
    println!("Listening for milter connection");

    let mut milter = PrintBodyMilter::default();
    let mut server = Server::default_postfix(&mut milter);

    loop {
        println!("==============");
        let (stream, _socket_addr) = listener
            .accept()
            .await
            .into_diagnostic()
            .wrap_err("Failed accepting connection")?;

        server
            .handle_connection(&mut stream.compat())
            .await
            .expect("Failed handling milter connection");
    }
}
