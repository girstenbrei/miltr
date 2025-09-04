//! A milter that prints callback arguments and macros for each stage.

use async_trait::async_trait;
use std::env;
use tokio::net::TcpListener;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use miltr_common::{
    actions::{Action, Continue},
    commands::{Body, Connect, Header, Helo, Macro, Mail, Recipient, Unknown},
    modifications::ModificationResponse,
    optneg::OptNeg,
};
use miltr_server::{Error, Milter, Server};

struct PrintMilter;

#[async_trait]
impl Milter for PrintMilter {
    type Error = &'static str;
    async fn option_negotiation(&mut self, opt_neg: OptNeg) -> Result<OptNeg, Error<Self::Error>> {
        println!("\n======== NEGOTIATE ========");
        println!("  opts received: {opt_neg:#?}");
        let opts = OptNeg::default();
        println!("  opts sent back: {opts:#?}");
        Ok(opts)
    }

    async fn connect(&mut self, connect_info: Connect) -> Result<Action, Self::Error> {
        println!("\n======== CONNECT ========");
        println!("  hostname: {}", connect_info.hostname());
        println!(
            "  socket_info: {}:{:?}",
            connect_info.address(),
            connect_info.port
        );
        println!("  family: {:?}", connect_info.family);
        Ok(Continue.into())
    }

    async fn helo(&mut self, helo: Helo) -> Result<Action, Self::Error> {
        println!("\n======== HELO ========");
        println!("  hostname: {}", helo.helo());
        Ok(Continue.into())
    }

    async fn mail(&mut self, mail: Mail) -> Result<Action, Self::Error> {
        println!("\n======== MAIL ========");
        println!("  sender: {}", mail.sender());
        for arg in mail.esmtp_args() {
            println!("  esmtp_args: {arg}");
        }
        Ok(Continue.into())
    }

    async fn rcpt(&mut self, recipient: Recipient) -> Result<Action, Self::Error> {
        println!("\n======== RCPT ========");
        println!("  recipient: {:?}", recipient.recipient());
        for arg in recipient.esmtp_args() {
            println!("  esmtp_args: {arg}");
        }
        Ok(Continue.into())
    }

    async fn data(&mut self) -> Result<Action, Self::Error> {
        println!("\n======== DATA ========");
        Ok(Continue.into())
    }

    async fn header(&mut self, header: Header) -> Result<Action, Self::Error> {
        println!("\n======== HEADER ========");
        println!("  name: {}", header.name());
        println!("  value: {}", header.value());
        Ok(Continue.into())
    }

    async fn end_of_header(&mut self) -> Result<Action, Self::Error> {
        println!("\n======== EOH ========");
        Ok(Continue.into())
    }

    async fn body(&mut self, body: Body) -> Result<Action, Self::Error> {
        println!("\n======== BODY ========");
        println!("  body part: {}", String::from_utf8_lossy(body.as_bytes()));
        Ok(Continue.into())
    }

    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        println!("\n======== END OF BODY ========");
        Ok(ModificationResponse::empty_continue())
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        println!("\n======== ABORT ========");
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), Self::Error> {
        println!("\n======== QUIT ========");
        Ok(())
    }

    async fn quit_nc(&mut self) -> Result<(), Self::Error> {
        println!("\n======== QUIT NEXT CONNECTION ========");
        Ok(())
    }

    async fn unknown(&mut self, cmd: Unknown) -> Result<Action, Self::Error> {
        println!("\n======== UNKNOWN ========");
        println!("  Raw: {cmd:?}");
        Ok(Continue.into())
    }

    async fn macro_(&mut self, macro_: Macro) -> Result<(), Self::Error> {
        println!("\n======== MACRO ========");
        println!(
            "  code: {}",
            char::from_u32(u32::from(macro_.code)).unwrap()
        );
        for (key, value) in macro_.macros() {
            println!(
                "  macro - {}:{}",
                String::from_utf8_lossy(key),
                String::from_utf8_lossy(value)
            );
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let addr = env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to addr");
    println!("\n======== Bound to socket ========");

    let mut milter = PrintMilter;
    let mut server = Server::default_postfix(&mut milter);

    loop {
        println!();
        println!("=========================================");
        println!("======== Awaiting new connection ========");
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
