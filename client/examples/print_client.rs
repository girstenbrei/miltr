//! A simple example milter client

use std::env;

use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use miette::{IntoDiagnostic, Result};

use miltr_client::Client;
use miltr_common::{
    commands::{Connect, Family, Header},
    optneg::OptNeg,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let addr = env::var("CONNECT_ADDR").unwrap_or("127.0.0.1:8080".to_string());
    let mut stream = TcpStream::connect(&addr)
        .await
        .expect("Failed connecting to milter server")
        .compat();

    println!("Opened TCP connection");
    let options = OptNeg::default();
    let client = Client::new(options);
    let mut connection = client
        .connect_via(&mut stream)
        .await
        .expect("Failed to setup connection");

    println!("Did option negotiation, ready for commands");
    connection
        .connect(Connect::new(
            "localhost".as_bytes(),
            Family::Inet,
            None,
            "127.0.0.1".as_bytes(),
        ))
        .await
        .into_diagnostic()?;
    connection
        .helo("localhost".as_bytes())
        .await
        .into_diagnostic()?;
    connection
        .mail("sender@test.local".as_bytes())
        .await
        .into_diagnostic()?;
    connection
        .recipient("rcpt@test.local".as_bytes())
        .await
        .into_diagnostic()?;
    connection.data().await.into_diagnostic()?;
    connection
        .header(Header::new("X-Header".as_bytes(), "My value".as_bytes()))
        .await
        .into_diagnostic()?;
    connection.end_of_header().await.into_diagnostic()?;
    connection
        .body("A very simple mail body".as_bytes())
        .await
        .into_diagnostic()?;

    println!("Commands sent, awaiting modification actions");
    let modification_response = connection.end_of_body().await.into_diagnostic()?;

    println!("Received modification actions:");
    for action in modification_response.modifications() {
        println!("{action:?}");
    }
    println!("Final action: {:?}", modification_response.final_action());

    connection.quit().await.into_diagnostic()?;

    Ok(())
}
