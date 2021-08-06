mod tika_document;
mod tui_app;
mod util;
mod xapian_utils;

use crate::tika_document::{parse_file, TikaDocument};
use crate::util::glob_files;
use clap::{App, Arg, ArgMatches, SubCommand};
use color_eyre::Report;
use xapian_rusty::{Document, Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN};

fn setup<'a>(default_config_file: &str) -> Result<ArgMatches, Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    let cli = App::new("tika")
        .version("1.0")
        .author("Steve <steve@little-fluffy.cloud>")
        .about("Things I Know About: Zettlekasten-like Markdown+FrontMatter Indexer and query tool")
        .arg(
            Arg::with_name("config")
                .short("c")
                .value_name("FILE")
                .help(
                    format!(
                        "Point to a config TOML file, defaults to `{}`",
                        default_config_file
                    )
                    .as_str(),
                )
                .default_value(&default_config_file)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("update-index")
                .short("i")
                .help("Index data rather than querying the DB"),
        )
        .arg(
            Arg::with_name("source")
                .short("s")
                .value_name("DIRECTORY")
                .help("Glob path to markdown files to load")
                .takes_value(true),
        )
        .subcommand(
            SubCommand::with_name("query")
                .about("Query the index")
                .arg(Arg::with_name("query").required(true).help("Query string")),
        )
        .get_matches();

    tui_app::setup_panic();

    Ok(cli)
}

fn main() -> Result<(), Report> {
    let default_config_file = shellexpand::tilde("~/.config/tika/tika.toml");
    let cli = setup(&default_config_file)?;

    // If requested, reindex the data
    if cli.occurrences_of("update-index") > 0 {
        let mut db = WritableDatabase::new("mydb", BRASS, DB_CREATE_OR_OPEN)?;
        let mut tg = TermGenerator::new()?;
        let mut stemmer = Stem::new("en")?;
        tg.set_stemmer(&mut stemmer)?;

        // TODO is there a rustier way to do this?
        for entry in glob_files(
            &cli.value_of("config").unwrap(),
            cli.value_of("source"),
            cli.occurrences_of("v") as i8,
        )
        .expect("Failed to read glob pattern")
        {
            match entry {
                // TODO convert this to iterator style using map/filter
                Ok(path) => {
                    if let Ok(tikadoc) = parse_file(&path) {
                        update_index(&mut db, &mut tg, &tikadoc)?;
                        if cli.occurrences_of("v") > 0 {
                            println!("✅ {}", tikadoc.filename);
                        }
                    } else {
                        eprintln!("❌ Failed to load file {}", path.display());
                    }
                }

                Err(e) => eprintln!("❌ {:?}", e),
            }
        }

        db.commit()?;
    }

    let mut iter = IntoIterator::into_iter(tui_app::interactive_query()?); // strings is moved here
    while let Some(s) = iter.next() {
        // next() moves a string out of the iter
        println!("{}", s);
    }

    Ok(())
}

fn update_index(
    db: &mut WritableDatabase,
    tg: &mut TermGenerator,
    tikadoc: &TikaDocument,
) -> Result<(), Report> {
    // Create a new Xapian Document to store attributes on the passed-in TikaDocument
    let mut doc = Document::new()?;
    tg.set_document(&mut doc)?;

    tg.index_text_with_prefix(&tikadoc.author, "A")?;
    tg.index_text_with_prefix(&tikadoc.date_str()?, "D")?;
    tg.index_text_with_prefix(&tikadoc.filename, "F")?;
    tg.index_text_with_prefix(&tikadoc.full_path.clone().into_string().unwrap(), "F")?;
    tg.index_text_with_prefix(&tikadoc.title, "S")?;
    tg.index_text_with_prefix(&tikadoc.subtitle, "XS")?;
    for tag in &tikadoc.tags {
        tg.index_text_with_prefix(&tag, "K")?;
    }

    tg.index_text(&tikadoc.body)?;

    // Convert the TikaDocument into JSON and set it in the DB for retrieval later
    doc.set_data(&serde_json::to_string(&tikadoc).unwrap())?;

    let id = "Q".to_owned() + &tikadoc.filename;
    doc.add_boolean_term(&id)?;
    db.replace_document(&id, &mut doc)?;

    Ok(())
}
