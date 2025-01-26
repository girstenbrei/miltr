use escargot::CargoRun;
use miette::{miette, Context, IntoDiagnostic, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

fn build_artifacts() -> Result<(CargoRun, CargoRun)> {
    println!("Building the server");
    let server = escargot::CargoBuild::new()
        .current_release()
        .current_target()
        .manifest_path("./server/Cargo.toml")
        .example("print_server")
        .run()
        .into_diagnostic()
        .wrap_err("Failed building server")?;

    println!("Build the client");
    let client = escargot::CargoBuild::new()
        .current_release()
        .current_target()
        .manifest_path("./client/Cargo.toml")
        .example("print_client")
        .run()
        .into_diagnostic()
        .wrap_err("Failed building client")?;

    Ok((server, client))
}

fn run_client(client: CargoRun) -> Result<()> {
    let client_exit_status = client
        .command()
        .status()
        .into_diagnostic()
        .wrap_err("Failed running client print example")?;

    if !client_exit_status.success() {
        return Err(miette!("Client failed with status {}", client_exit_status));
    }

    Ok(())
}

#[test]
fn client_v_server() {
    // Build
    let (server, client) = build_artifacts().expect("Failed building");

    // Run server and client
    let mut server = server
        .command()
        .spawn()
        .expect("Failed running print example");
    let client_res = run_client(client);

    // Shutdown server to kill sub process without zombie
    signal::kill(Pid::from_raw(server.id() as i32), Signal::SIGTERM)
        .expect("Failed SIGTERM server process in test");
    let _server_exit_status = server.wait().expect("Failed waiting for server shutdown");

    // Report on the client success
    client_res.expect("Failed to successfully run client");
}
