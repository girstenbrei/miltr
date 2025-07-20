use std::{
    fmt::Write as _,
    net::SocketAddr,
    os::unix,
    path::{Path, PathBuf},
};

use lettre::transport::smtp::response::Response;
use miette::{miette, Context, IntoDiagnostic, Result};
use tokio::{
    fs::{self, create_dir_all, remove_dir_all, write, File},
    io::AsyncWriteExt,
    process::Command,
};
use walkdir::WalkDir;

use crate::utils::PortGuard;

pub struct PostfixInstance {
    name: String,
    port: u16,
    milter_addr: SocketAddr,
    config_dir: PathBuf,
    spool_dir: PathBuf,
    data_dir: PathBuf,
}

impl PostfixInstance {
    pub async fn setup(name: &str, milter_addr: SocketAddr) -> Result<PostfixInstance> {
        let name = format!("postfix-{name}");
        let port = PortGuard::lock()
            .await
            .wrap_err("Failed locking postguard")?
            .port()
            .await
            .wrap_err("Failed to retrieve smtp port")?;
        println!("Setup '{name}' on :{port}");

        let config_dir = PathBuf::from(format!("/etc/{name}"));
        let spool_dir = PathBuf::from(format!("/var/spool/{name}"));
        let data_dir = PathBuf::from(format!("/var/lib/{name}"));

        let mut instance = Self {
            name,
            port,
            milter_addr,
            config_dir,
            spool_dir,
            data_dir,
        };

        // If the instance already exists, destroy it
        let pid_file = instance.spool_dir.join("pid/master.pid");
        if pid_file.is_file() {
            let pid = fs::read_to_string(&pid_file)
                .await
                .into_diagnostic()
                .wrap_err("Failed reading pid file")?;

            let proc_pid = PathBuf::from_iter(["/proc", pid.trim()]);
            if proc_pid.exists() {
                println!("Stopping running instance with pid '{}'", pid.trim());
                instance.stop().await?;
            }
        }

        let log_file = instance.log_file_name();
        File::options()
            .truncate(true)
            .write(true)
            .create(true)
            .open(log_file)
            .await
            .into_diagnostic()
            .wrap_err("Failed creating or truncating logfile")?;
        if instance.spool_dir.is_dir() {
            remove_dir_all(&instance.spool_dir)
                .await
                .into_diagnostic()
                .wrap_err("Failed removing spool dir")?;
        }
        if instance.data_dir.is_dir() {
            remove_dir_all(&instance.data_dir)
                .await
                .into_diagnostic()
                .wrap_err("Failed removing data dir")?;
        }

        instance.create_dirs_and_config().await?;

        status(
            "postfix",
            &["-c", &instance.config_dir.display().to_string(), "start"],
        )
        .await
        .wrap_err("Failed running 'postfix start'")?;

        Ok(instance)
    }

    fn log_file_name(&self) -> String {
        format!("/var/log/mail-{}.log", self.name)
    }

    pub async fn log_file_content(&self) -> Result<String> {
        fs::read_to_string(self.log_file_name())
            .await
            .into_diagnostic()
            .wrap_err("Failed to read postfix log file")
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    async fn create_dirs_and_config(&mut self) -> Result<()> {
        create_and_own(&self.config_dir, 0, 0)
            .await
            .wrap_err("Failed config dir setup")?;
        create_and_own(&self.spool_dir, 0, 0)
            .await
            .wrap_err("Failed spool dir setup")?;
        create_and_own(&self.data_dir, 100, 103)
            .await
            .wrap_err("Failed data dir setup")?;

        // Postconf baseline
        self.render_cf("main", |mut main_cf| {
            main_cf = main_cf.replace("/var/log/mail.log", &self.log_file_name());
            let _ = writeln!(main_cf, "data_directory = {}", &self.data_dir.display());
            let _ = writeln!(main_cf, "queue_directory = {}", &self.spool_dir.display());
            main_cf = main_cf.replace(
                "transport_maps = hash:/etc/postfix/transport",
                &format!(
                    "transport_maps = hash:{}/transport",
                    &self.config_dir.display()
                ),
            );
            main_cf = main_cf.replace(
                "smtpd_milters = inet:127.0.0.1:8080",
                &format!("smtpd_milters = inet:{}", self.milter_addr),
            );
            main_cf
        })
        .await
        .wrap_err("Failed to configure postfix instance")?;

        // SMTP input
        // Disable the standard smtp listener
        self.render_cf("master", |mut main_cf| {
            main_cf = main_cf.replace(
                "smtp      inet  n       -       y       -       -       smtpd",
                "#smtp      inet  n       -       y       -       -       smtpd",
            );
            let _ = writeln!(
                main_cf,
                "{}       inet  n       -       y       -       -       smtpd",
                self.port
            );
            main_cf
        })
        .await
        .wrap_err("Failed updating instance master.cf")?;

        // Add transport output to smtpsink all mails
        write(self.config_dir.join("transport"), "* smtp:127.0.0.1:2525\n")
            .await
            .into_diagnostic()
            .wrap_err("Failed to create transport map")?;

        let cmd = Command::new("postmap")
            .args(["-c", &self.config_dir.display().to_string(), "./transport"])
            .current_dir(&self.config_dir)
            .status()
            .await
            .into_diagnostic()?;

        if !cmd.success() {
            return Err(miette!("'postmap' failed to generate transport table"));
        }

        Ok(())
    }

    async fn render_cf(&self, config: &str, updater: impl Fn(String) -> String) -> Result<()> {
        let cf_in_path = format!("/etc/postfix/{config}.cf");
        let mut cf_content = fs::read_to_string(&cf_in_path)
            .await
            .into_diagnostic()
            .wrap_err("Failed to read in main.cf for instance")?;

        cf_content = updater(cf_content);

        let cf_out_path = self.config_dir.join(config).with_extension("cf");
        File::options()
            .truncate(true)
            .create(true)
            .write(true)
            .open(&cf_out_path)
            .await
            .into_diagnostic()
            .wrap_err("Failed open .cf file to write back config")?
            .write_all(cf_content.as_bytes())
            .await
            .into_diagnostic()
            .wrap_err("Failed to update .cf conf")?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        status(
            "postfix",
            &["-c", &self.config_dir.display().to_string(), "stop"],
        )
        .await
        .wrap_err("Failed running 'postfix stop'")?;

        Ok(())
    }

    pub async fn get_mail_content(&self, response: &Response) -> Result<String> {
        let id = response_to_id(response)?;

        let message_file = self.active_or_defered(id)?;

        let content = Command::new("postcat")
            .arg(message_file.display().to_string())
            .output()
            .await
            .into_diagnostic()
            .wrap_err("Failed postcat-ing message file")?
            .stdout;
        let content = String::from_utf8_lossy(&content).to_string();

        Ok(content)
    }

    fn active_or_defered(&self, id: &str) -> Result<PathBuf> {
        for typ in ["active", "deferred"] {
            let current_dir = self.spool_dir.join(typ);
            for entry in WalkDir::new(current_dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.metadata().unwrap().is_file())
            {
                let path = entry.path();

                if let Some(name) = path.file_name() {
                    if name.to_string_lossy() == id {
                        return Ok(path.to_path_buf());
                    }
                }
            }
        }

        Err(miette!("The message was not active or deferred by postfix"))
    }
}

async fn create_and_own(path: impl AsRef<Path>, pid: u32, gid: u32) -> Result<()> {
    create_dir_all(path.as_ref())
        .await
        .into_diagnostic()
        .wrap_err("Failed creating directory")?;

    unix::fs::chown(path, Some(pid), Some(gid))
        .into_diagnostic()
        .wrap_err("Failed owning created directory")?;

    Ok(())
}

async fn status(cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .into_diagnostic()?;

    if !output.status.success() {
        return Err(miette!(
            "'{} {}' returned {}:\nStderr:\n{}Stdout:\n{}",
            cmd,
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }

    Ok(())
}

fn response_to_id(response: &Response) -> Result<&str> {
    if !response.is_positive() {
        return Err(miette!(
            help = "Only mails which where succesfully sent can be content checked",
            "Provided response was not positive"
        ));
    }

    let id = response
        .first_line()
        .ok_or(miette!(
            "The provided response did not contain id information"
        ))?
        .split_ascii_whitespace()
        .last()
        .ok_or(miette!(
            "The provided response did not contain a postfix id"
        ))?;
    println!("Postifx id: {id}");

    Ok(id)
}

// #[tokio::test]
// async fn test_setup() {
//     let listener = TcpListener::bind("0.0.0.0:0")
//         .await
//         .expect("Failed to bind to addr");
//     let local_addr = listener.local_addr().expect("Failed to read local addr");
//     let postfix = PostfixInstance::setup("instance_fixture_test_setup", local_addr)
//         .await
//         .expect("Failed to configure postfix instance");
//     postfix.stop().await.expect("Failed stopping postfix");
// }
