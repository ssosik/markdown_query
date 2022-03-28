mod interactive;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use color_eyre::Report;
use crossbeam_channel::Sender;
use cursive::traits::{Nameable, Resizable, Scrollable};
use cursive::view::ScrollStrategy;
use cursive::views::{EditView, LinearLayout, Panel};
use cursive::Cursive;
use cursive::{
    align::HAlign,
    event::{EventResult, Key},
    traits::With,
    view::scroll::Scroller,
    views::{Dialog, OnEventView, TextView},
};
use log::{debug, error};
use markdown_query::document;
use std::ffi::OsStr;
use walkdir::WalkDir;
use xapian_rusty::{Database, Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN};

pub enum UICommand {
    UpdateUsername(String),
    SendMessage(String),
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Cli {
    /// Set level of verbosity
    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,

    /// Specify a PAGER to use when viewing markdown
    #[clap(long, env = "PAGER", default_value = "less")]
    pager: String,

    /// Specify an EDITOR to use when editing markdown
    #[clap(long, env = "EDITOR", default_value = "vi")]
    editor: String,

    /// Specify where to write the DB to
    #[clap(
        short,
        long,
        parse(from_os_str),
        value_name = "XAPIAN DB DIR",
        default_value = "~/.mdq-data"
    )]
    db_path: Box<OsStr>,

    #[clap(subcommand)]
    subcommand: Option<Subcommands>,
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "snake_case")]
enum Subcommands {
    /// WIP Terminal test using cursive
    Cursive,

    /// Re-index data
    Update {
        /// Directories to search recursively for markdown content
        paths: Vec<String>,
    },

    /// Specify a starting query for interactive query mode
    Query {
        /// Query string
        query: String,
    },
}

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}

fn main() -> Result<(), Report> {
    // Parse CLI Arguments
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let db_path: String = shellexpand::tilde(cli.db_path.to_str().unwrap()).into();

    setup()?;

    match cli.subcommand {
        Some(Subcommands::Update { ref paths }) => {
            let mut db = WritableDatabase::new(&db_path, BRASS, DB_CREATE_OR_OPEN)
                .expect("Could not open db for writing");
            let mut tg = TermGenerator::new()?;
            let mut stemmer = Stem::new("en")?;
            tg.set_stemmer(&mut stemmer)?;

            for path in paths {
                let walker = WalkDir::new(path).into_iter();
                for entry in walker.filter_entry(|e| {
                    !e.file_name()
                        .to_str()
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false)
                }) {
                    match entry {
                        Ok(path) => {
                            let path = path.path();
                            if path.extension().is_none() || path.extension().unwrap() != "md" {
                                continue;
                            }
                            if let Ok(doc) = document::Document::parse_file(path) {
                                doc.update_index(&mut db, &mut tg)?;
                                debug!("✅ {}", path.display());
                            } else {
                                error!("❌ Failed to load file {}", path.display());
                            }
                        }

                        Err(e) => error!("❌ {:?}", e),
                    }
                }

                db.commit()?;
            }
        }
        None => {
            interactive::setup_panic();
            let db = Database::new_with_path(&db_path, DB_CREATE_OR_OPEN)?;
            let iter = IntoIterator::into_iter(interactive::query(
                db,
                cli.pager,
                cli.editor,
                String::from(""),
            )?); // strings is moved here
            for s in iter {
                // next() moves a string out of the iter
                println!("{}", s);
            }
        }
        Some(Subcommands::Cursive) => {
            // Read some long text from a file.
            let content = include_str!("../Makefile");

            let mut siv = cursive::default();

            // We can quit by pressing q
            siv.add_global_callback('q', |s| s.quit());

            use crossbeam_channel::unbounded;
            let (ui_tx, ui_rx) = unbounded::<UICommand>();

            // The text is too long to fit on a line, so the view will wrap lines,
            // and will adapt to the terminal size.
            siv.add_fullscreen_layer(
                LinearLayout::horizontal()
                    .child(
                        LinearLayout::vertical()
                            .child(
                                Panel::new(
                                    LinearLayout::vertical()
                                        .with_name("chat_inner")
                                        .full_height()
                                        .full_width()
                                        .scrollable()
                                        .scroll_strategy(ScrollStrategy::StickToBottom),
                                )
                                .title("arpchat")
                                .with_name("chat_panel")
                                .full_height()
                                .full_width(),
                            )
                            .child(
                                Panel::new(
                                    EditView::new()
                                        .on_submit(move |siv, msg| {
                                            siv.call_on_name("input", |input: &mut EditView| {
                                                input.set_content("");
                                            });
                                            ui_tx
                                                .send(UICommand::SendMessage(msg.to_string()))
                                                .unwrap();
                                        })
                                        .with_name("input"),
                                )
                                .full_width(),
                            )
                            .full_width(),
                    )
                    .child(
                        Panel::new(
                            LinearLayout::vertical()
                                .with_name("presences")
                                .full_height()
                                .full_width()
                                .scrollable()
                                .scroll_strategy(ScrollStrategy::StickToBottom),
                        )
                        .title("online users")
                        .full_height()
                        .fixed_width(32),
                    ),
            );
            siv.run();
        }
        Some(Subcommands::Query { query }) => {
            interactive::setup_panic();

            let db = Database::new_with_path(&db_path, DB_CREATE_OR_OPEN)?;
            let iter =
                IntoIterator::into_iter(interactive::query(db, cli.pager, cli.editor, query)?); // strings is moved here
            for s in iter {
                // next() moves a string out of the iter
                println!("{}", s);
            }
        }
    }

    Ok(())
}
