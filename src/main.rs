use clap::{App, AppSettings, Arg, SubCommand};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
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

fn single_command(command: &str) -> anyhow::Result<()> {
    let res = send_msg(command);

    if res.is_ok() {
        log::info!("Success!");
    }

    res
}

fn refresh() {
    log::info!("Refreshing...");

    if let Err(e) = send_msg("background_refresh") {
        log::error!("An error occurred: {}", e);
    }
    else {
        log::info!("Done")
    }
}

fn watch(mut path: PathBuf, delay: Duration) -> anyhow::Result<()> {
    log::info!("Watching project at {}", path.display());

    path.push("Assets");

    if !path.is_dir() {
        return Err(anyhow::format_err!(
            "Assets dir not found at {}. Are you sure that this is a valid Unity project?",
            path.display()
        ));
    }

    // Create a channel to receive the events.
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = watcher(tx, delay)?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    loop {
        // observe the events that imply that a file is actually changed
        match rx.recv()? {
            DebouncedEvent::NoticeWrite(_) => {}
            DebouncedEvent::NoticeRemove(_) => {}
            DebouncedEvent::Create(_) => refresh(),
            DebouncedEvent::Write(_) => refresh(),
            DebouncedEvent::Chmod(_) => {}
            DebouncedEvent::Remove(_) => refresh(),
            DebouncedEvent::Rename(_, _) => refresh(),
            DebouncedEvent::Rescan => {}
            DebouncedEvent::Error(e, _) => Err(e)?,
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
                .about("Rebuild all scripts. Only compatible with Unity 2019.3+"),
        )
        .subcommand(
            SubCommand::with_name("watch")
                .about("Automatically calls refresh if anything under /Assets/ changes")
                .arg(
                    Arg::with_name("PROJECT_DIR")
                        .help("Path to the Unity project to watch")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("delay")
                        .short("d")
                        .long("delay")
                        .value_name("SECONDS")
                        .help("Only start a refresh after this many seconds")
                        .default_value("4")
                        .takes_value(true),
                ),
        )
        .setting(AppSettings::ArgRequiredElseHelp);

    let matches = app.get_matches();

    if let Some(_matches) = matches.subcommand_matches("play") {
        single_command("play")?;
    } else if let Some(_matches) = matches.subcommand_matches("stop") {
        single_command("stop")?;
    } else if let Some(_matches) = matches.subcommand_matches("refresh") {
        single_command("refresh")?;
    } else if let Some(_matches) = matches.subcommand_matches("build") {
        single_command("build")?;
    } else if let Some(matches) = matches.subcommand_matches("watch") {
        let path = matches
            .value_of("PROJECT_DIR")
            .expect("Clap should require this");

        let path = PathBuf::from(path);

        let delay: u64 = matches.value_of("delay").unwrap().parse()?;

        watch(path, Duration::from_secs(delay))?;
    }

    Ok(())
}
