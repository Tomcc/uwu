use std::net::UdpSocket;

use clap::{App, AppSettings, SubCommand};

const ADDR: &str = "127.0.0.1:38911";
const UNITY_ADDR: &str = "127.0.0.1:38910";

fn send_msg(msg: &str) -> anyhow::Result<()> {
    UdpSocket::bind(ADDR)?.send_to(msg.as_bytes(), UNITY_ADDR)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let log_env = env_logger::Env::new().default_filter_or("info");
    env_logger::init_from_env(log_env);

    let app = App::new("UWU - Unity Workflow for UDP")
        // .version("1.0")
        // .author("Kevin K. <kbknapp@gmail.com>")
        // .about("Does awesome things")
        .subcommand(
            SubCommand::with_name("play").about("Make any listening Editor start Play mode"),
        )
        .subcommand(
            SubCommand::with_name("stop").about("Stop current Play mode"),
        )
        .subcommand(
            SubCommand::with_name("refresh").about("Make any listening Editor refresh its assets, eg. rebuild code"),
        )
        .setting(AppSettings::ArgRequiredElseHelp);

    let matches = app.get_matches();

    if let Some(_matches) = matches.subcommand_matches("play") {
        send_msg("play")?;
    }
    else if let Some(_matches) = matches.subcommand_matches("stop") {
        send_msg("stop")?;
    }
    else if let Some(_matches) = matches.subcommand_matches("refresh") {
        send_msg("refresh")?;
    }

    log::info!("Done!");

    Ok(())
}
