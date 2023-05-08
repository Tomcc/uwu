mod id64;

use anyhow::bail;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand,
};
use id64::Id64;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

const UNITY_ADDR_STR: &str = "127.0.0.1:38910";
const UNITY_ADDR: Lazy<SocketAddr> =
    Lazy::new(|| SocketAddr::from_str(UNITY_ADDR_STR).expect("Failed to parse UNITY_ADDR_STR"));
// very short timeout, this is supposed to be used over localhost
const TIMEOUT: Duration = Duration::from_secs(5);

static SOCKET: Lazy<UdpSocket> =
    Lazy::new(|| UdpSocket::bind("127.0.0.1:0").expect("Failed to bind to a random port"));

#[derive(Debug, Serialize)]
enum Command {
    Play,
    CheckAlive,
    Stop,
    Refresh,
    BackgroundRefresh,
    Build,
}

#[derive(Debug, Serialize)]
struct Request {
    id: Id64,
    cmd: Command,
}

#[derive(Debug, Deserialize)]
enum Response {
    Success,
    Error,
    Wait,
}

// Send one message over UDP, and retry if it times out until ACK is received
// This is needed because Unity may be recreating the socket, and the message could get lost
fn send_reliable_blocking(request: &Request) -> anyhow::Result<()> {
    SOCKET.set_read_timeout(Some(TIMEOUT))?;

    let msg = serde_json::to_vec(request)?;

    log::debug!("Sending {}", String::from_utf8_lossy(&msg));

    // repeat until acknowledged
    let mut recv_buf = [0; 1024];
    loop {
        // send the message
        SOCKET.send_to(&msg, &*UNITY_ADDR)?;

        // receive the response
        match SOCKET.recv_from(&mut recv_buf) {
            Ok((size, _src)) => {
                // deserialize the response
                let response: Response = serde_json::from_slice(&recv_buf[..size])?;

                match response {
                    // Success means that we're done
                    Response::Success => {
                        log::debug!("Response received");
                        return Ok(());
                    }
                    // Wait means that we should receive Success or Error later.
                    // Break the loop and wait for the next message
                    Response::Wait => {
                        break;
                    }
                    Response::Error => {
                        bail!("Unity-side error");
                    }
                }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock => {
                    log::debug!("No ACK received within timeout, retrying");
                }
                _ => {
                    return Err(e.into());
                }
            },
        }
    }

    log::info!("Waiting for Unity...");

    // wait for the final message
    SOCKET.set_read_timeout(None)?;
    let (size, _src) = SOCKET.recv_from(&mut recv_buf)?;

    // deserialize the response
    let response: Response = serde_json::from_slice(&recv_buf[..size])?;

    match response {
        // Success means that we're done
        Response::Success => {
            log::debug!("Final response received");
            return Ok(());
        }
        Response::Error => {
            bail!("Unity-side error");
        }
        // Wait means that we should receive Success or Error later.
        // Break the loop and wait for the next message
        Response::Wait => {
            bail!("Unexpected Wait response");
        }
    }
}

fn single_command(command: Command) -> anyhow::Result<()> {
    let req = Request {
        // pick a random ID so that the server can keep track of mistaken resends
        id: Id64::random(),
        cmd: command,
    };

    send_reliable_blocking(&req)
}

fn watch(mut path: PathBuf, delay: Duration) -> anyhow::Result<()> {
    println!("Watching project at {}", path.display());

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

    let refresh = || {
        println!("Refreshing");

        // handle some errors by breaking and reconnecting, otherwise return the error
        if let Err(e) = single_command(Command::BackgroundRefresh) {
            log::error!("An error occurred: {}", e);
        }

        // NOTE: this may kill the server if scripts are reloaded,
        // but as it is a background operation, we don't need to wait until it restarts;
        // we can just queue more refresh requests
    };

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
    let app = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("verbose")
                .help("Prints more log messages. Same as RUST_LOG=debug")
                .short("v")
                .long("verbose")
                .takes_value(false),
        )
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
                        .default_value("1")
                        .takes_value(true),
                ),
        )
        .setting(AppSettings::SubcommandRequiredElseHelp);
    let matches = app.get_matches();

    let log_level = if matches.is_present("verbose") {
        "debug"
    } else {
        "info"
    };

    let log_env = env_logger::Env::new().default_filter_or(log_level);
    env_logger::init_from_env(log_env);

    if let Some(_matches) = matches.subcommand_matches("play") {
        // Play is complex. First, we need to request to enter play mode, which will succeed
        // immediately, but it will kill the Unity client, so it cannot return WAIT.
        single_command(Command::Play)?;

        // Then, we need to wait until the Unity client has restarted and is ready to receive
        single_command(Command::CheckAlive)?;

        println!("ok");
    } else if let Some(_matches) = matches.subcommand_matches("stop") {
        single_command(Command::Stop)?;

        println!("ok");
    } else if let Some(_matches) = matches.subcommand_matches("refresh") {
        single_command(Command::Refresh)?;

        // same as Play, wait until our client is ready to receive
        single_command(Command::CheckAlive)?;

        println!("ok");
    } else if let Some(_matches) = matches.subcommand_matches("build") {
        single_command(Command::Build)?;

        println!("ok");
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
