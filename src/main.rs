use clap::{App, Arg, SubCommand};
use color_eyre::Report;
use dirs::home_dir;
use markdown_query::{document, tui_app};
use walkdir::WalkDir;
use xapian_rusty::{Database, Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN};

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}

fn main() -> Result<(), Report> {
    setup()?;

    let mut db_path = home_dir().unwrap();
    db_path.push(".mdq-data");

    let cli = App::new("mdq")
        .version("1.0")
        .author("Steve <steve@little-fluffy.cloud>")
        .about("mdq: Markdown+FrontMatter Indexer and query tool")
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("db-path")
                .help("Specify where to write the DB to")
                .default_value(db_path.to_str().unwrap()),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Specify a path to a directory (searched recursively) containing markdown files to parse")
                .arg(
                    Arg::with_name("path")
                        .help("directory to recursively search")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("query")
                .about("Specify a starting query for interactive query mode")
                .arg(Arg::with_name("query").required(true).help("Query string")),
        )
        .get_matches();

    let verbosity = cli.occurrences_of("v");
    let db_path = cli.value_of("db-path").unwrap();

    // If requested, reindex the data
    if let Some(cli) = cli.subcommand_matches("update") {
        let mut db = WritableDatabase::new(db_path, BRASS, DB_CREATE_OR_OPEN)?;
        let mut tg = TermGenerator::new()?;
        let mut stemmer = Stem::new("en")?;
        tg.set_stemmer(&mut stemmer)?;

        let walker = WalkDir::new(cli.value_of("path").unwrap()).into_iter();
        for entry in walker.filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| s.starts_with("."))
                .unwrap_or(false)
        }) {
            match entry {
                Ok(path) => {
                    let path = path.path();
                    if path.extension().is_none() || path.extension().unwrap() != "md" {
                        continue;
                    }
                    if let Ok(doc) = document::Document::parse_file(&path) {
                        doc.update_index(&mut db, &mut tg)?;
                        if verbosity > 0 {
                            println!("✅ {}", doc.filename);
                        }
                    } else {
                        eprintln!("❌ Failed to load file {}", path.display());
                    }
                }

                Err(e) => eprintln!("❌ {:?}", e),
            }
        }

        db.commit()?;
    } else {
        // Else, query the DB
        tui_app::setup_panic();

        let db = Database::new_with_path(db_path, DB_CREATE_OR_OPEN)?;
        let iter = IntoIterator::into_iter(tui_app::interactive_query(db)?); // strings is moved here
        for s in iter {
            // next() moves a string out of the iter
            println!("{}", s);
        }
    }

    Ok(())
}
