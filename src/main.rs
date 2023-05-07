use clap::{
    crate_authors, crate_description, crate_name, crate_version, App, AppSettings, Arg, SubCommand,
};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    str::FromStr,
    thread::sleep,
    time::Duration,
};
use thiserror::Error;

const UNITY_ADDR_STR: &str = "127.0.0.1:38910";
const UNITY_ADDR: Lazy<SocketAddr> =
    Lazy::new(|| SocketAddr::from_str(UNITY_ADDR_STR).expect("Failed to parse UNITY_ADDR_STR"));
// very short timeout, this is supposed to be used over localhost
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Error, Debug)]
enum UnitySocketError {
    #[error("Send IO error: {0}")]
    FailedToSend(std::io::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Unity is responding with an empty string")]
    Empty,

    #[error("Unity failed to respond {0}")]
    FailedToRecv(std::io::Error),

    #[error("Something went wrong on Unitys side")]
    UnityError,

    #[error("Unknown response received: {0}")]
    UnknownResponse(String),

    #[error("Socket disconnected")]
    Disconnected,
}

type UnitySocketResult = Result<(), UnitySocketError>;

#[derive(Debug, Default)]
struct UnitySocket {
    stream: RefCell<Option<TcpStream>>,
}

impl UnitySocket {
    fn reconnect_loop(&self) {
        loop {
            // if we were connected, close the connection
            if let Some(stream) = self.stream.replace(None) {
                drop(stream);
            }

            // Try to connect
            let res = TcpStream::connect_timeout(&*UNITY_ADDR, TIMEOUT);

            if let Ok(stream) = res {
                self.stream.replace(Some(stream));
                return;
            }

            sleep(Duration::from_secs(3));
        }
    }

    fn try_send(&self, msg: &str) -> UnitySocketResult {
        let stream = self.stream.borrow_mut();
        let mut stream = match stream.as_ref() {
            Some(stream) => stream,
            None => {
                return Err(UnitySocketError::Disconnected);
            }
        };

        stream
            .write(msg.as_bytes())
            .map_err(|e| UnitySocketError::FailedToSend(e))?;

        let mut buf = String::new();
        stream
            .read_to_string(&mut buf)
            .map_err(|e| UnitySocketError::FailedToRecv(e))?;

        if buf == "OK" {
            Ok(())
        } else if buf.is_empty() {
            Err(UnitySocketError::Empty)
        } else if buf == "ERR" {
            Err(UnitySocketError::UnityError)
        } else {
            Err(UnitySocketError::UnknownResponse(buf))
        }
    }

    fn reconnect_loop_send(&self, msg: &str) -> anyhow::Result<()> {
        loop {
            self.reconnect_loop();

            // handle some errors by breaking and reconnecting, otherwise return the error
            match self.try_send(msg) {
                Ok(()) => return Ok(()),
                Err(UnitySocketError::Disconnected) => {}
                Err(UnitySocketError::FailedToRecv(_)) => {}
                Err(UnitySocketError::FailedToSend(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

fn single_command(command: &str) -> anyhow::Result<()> {
    let socket = UnitySocket::default();
    let res = socket.reconnect_loop_send(command);

    if res.is_ok() {
        log::info!("Success!");
    }

    res
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

    // Create the unity socket and try to connect eagerly
    let socket = UnitySocket::default();
    socket.reconnect_loop();

    // Create a channel to receive the events.
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = watcher(tx, delay)?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    let refresh = || {
        if let Err(e) = socket.try_send("background_refresh") {
            log::error!("An error occurred: {}", e);
        } else {
            log::debug!("Done")
        }
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
