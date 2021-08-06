use glob::{glob, Paths};
use std::{fs, io, io::Read, path::Path};
use toml::Value as tomlVal;

pub(crate) fn glob_files(
    cfg_file: &str,
    source: Option<&str>,
    verbosity: i8,
) -> Result<Paths, Box<dyn std::error::Error>> {
    let cfg_fh = fs::OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .open(cfg_file)?;
    let mut buf_reader = io::BufReader::new(cfg_fh);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;
    let toml_contents = contents.parse::<tomlVal>().unwrap();

    let source_glob = toml_contents
        .get("source-glob")
        .expect("Failed to find 'source-glob' heading in toml config")
        .as_str()
        .expect("Error taking source-glob value as string");

    let source = source.unwrap_or(source_glob);
    let glob_path = Path::new(&source);
    let glob_str = shellexpand::tilde(glob_path.to_str().unwrap());

    if verbosity > 0 {
        println!("Sourcing Markdown documents matching : {}", glob_str);
    }

    return Ok(glob(&glob_str).expect("Failed to read glob pattern"));
}

pub(crate) mod event {

    use std::io;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use termion::event::Key;
    use termion::input::TermRead;

    pub enum Event<I> {
        Input(I),
        Tick,
    }

    /// A small event handler that wrap termion input and tick events. Each event
    /// type is handled in its own thread and returned to a common `Receiver`
    pub struct Events {
        rx: mpsc::Receiver<Event<Key>>,
        input_handle: thread::JoinHandle<()>,
        tick_handle: thread::JoinHandle<()>,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Config {
        pub tick_rate: Duration,
    }

    impl Default for Config {
        fn default() -> Config {
            Config {
                tick_rate: Duration::from_millis(250),
            }
        }
    }

    impl Events {
        pub fn new() -> Events {
            Events::with_config(Config::default())
        }

        pub fn with_config(config: Config) -> Events {
            let (tx, rx) = mpsc::channel();
            let input_handle = {
                let tx = tx.clone();
                thread::spawn(move || {
                    let stdin = io::stdin();
                    for evt in stdin.keys() {
                        if let Ok(key) = evt {
                            if let Err(err) = tx.send(Event::Input(key)) {
                                eprintln!("{}", err);
                                return;
                            }
                        }
                    }
                })
            };
            let tick_handle = {
                thread::spawn(move || loop {
                    if let Err(err) = tx.send(Event::Tick) {
                        eprintln!("{}", err);
                        break;
                    }
                    thread::sleep(config.tick_rate);
                })
            };
            Events {
                rx,
                input_handle,
                tick_handle,
            }
        }

        pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
            self.rx.recv()
        }
    }
}
