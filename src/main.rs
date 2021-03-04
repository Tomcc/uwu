use clap::{App, AppSettings, SubCommand};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    str::FromStr,
    thread::sleep,
    time::Duration,
};

const UNITY_ADDR: &str = "127.0.0.1:38910";

// very short timeout, this is supposed to be used over localhost

fn send_msg(first_msg: &str) -> anyhow::Result<()> {
    let mut timeout: Duration = Duration::from_secs(5);
    let mut msg = first_msg;

    loop {
        let addr = SocketAddr::from_str(UNITY_ADDR)?;

        let mut stream = TcpStream::connect_timeout(&addr, timeout)?;

        log::debug!("Connected to {}", stream.peer_addr()?);

        // send the message
        stream.write(msg.as_bytes())?;

        // wait for an answer
        let mut buf = String::new();
        stream.read_to_string(&mut buf)?;

        if buf == "OK" {
            return Ok(());
        } else if buf == "RECONNECT" || buf.is_empty() {
            // don't return, loop and connect again with a larger timeout
            timeout = Duration::from_secs(30);
            msg = "confirm_restart";

            sleep(Duration::from_secs(3));
        } else if buf == "ERR" {
            return Err(anyhow::format_err!("Something went wrong"));
        } else {
            return Err(anyhow::format_err!("Unknown response received: {}", buf));
        }
    }
}

fn main() -> anyhow::Result<()> {
    let log_env = env_logger::Env::new().default_filter_or("info");
    env_logger::init_from_env(log_env);

    let app = App::new("UWU - Unity Workflow for UDP")
        // .version("1.0")
        // .author("Kevin K. <kbknapp@gmail.com>")
        // .about("Does awesome things")
        .subcommand(SubCommand::with_name("play").about("Start Play mode"))
        .subcommand(SubCommand::with_name("stop").about("Stop current Play mode"))
        .subcommand(SubCommand::with_name("refresh").about("Refresh all assets"))
        .subcommand(
            SubCommand::with_name("build")
                .about("Rebuild scripts. Only compatible with Unity 2019.3+"),
        )
        .setting(AppSettings::ArgRequiredElseHelp);

    let matches = app.get_matches();

    if let Some(_matches) = matches.subcommand_matches("play") {
        send_msg("play")?;
    } else if let Some(_matches) = matches.subcommand_matches("stop") {
        send_msg("stop")?;
    } else if let Some(_matches) = matches.subcommand_matches("refresh") {
        send_msg("refresh")?;
    } else if let Some(_matches) = matches.subcommand_matches("build") {
        send_msg("build")?;
    }

    log::info!("Success!");

    Ok(())
}
