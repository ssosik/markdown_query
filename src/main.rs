mod tui_app;
mod util;
mod xapian_utils;
mod xq_document;

use crate::util::glob_files;
use crate::xq_document::{parse_file, XqDocument};
use clap::{App, Arg, SubCommand};
use color_eyre::Report;
use dirs::home_dir;
use xapian_rusty::{
    Database, Document, Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN,
};

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
    db_path.push(".xq-data");

    let cli = App::new("xq")
        .version("1.0")
        .author("Steve <steve@little-fluffy.cloud>")
        .about("xq: Zettlekasten-like Markdown+FrontMatter Indexer and query tool")
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
                .about("Specify a path/glob pattern of matching files to index")
                .arg(
                    Arg::with_name("globpath") // And their own arguments
                        .help("the files to add")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("query")
                .about("Specify a starting query for interactive query mode")
                .arg(Arg::with_name("query").required(true).help("Query string")),
        )
        .get_matches();

    tui_app::setup_panic();

    let verbosity = cli.occurrences_of("v");
    let db_path = cli.value_of("db-path").unwrap();

    // If requested, reindex the data
    if let Some(cli) = cli.subcommand_matches("update") {
        let mut db = WritableDatabase::new(db_path, BRASS, DB_CREATE_OR_OPEN)?;
        let mut tg = TermGenerator::new()?;
        let mut stemmer = Stem::new("en")?;
        tg.set_stemmer(&mut stemmer)?;

        // TODO is there a rustier way to do this?
        for entry in glob_files(
            cli.value_of("globpath").unwrap(),
            cli.occurrences_of("v") as i8,
        )
        .expect("Failed to read glob pattern")
        {
            match entry {
                // TODO convert this to iterator style using map/filter
                Ok(path) => {
                    if let Ok(xqdoc) = parse_file(&path) {
                        update_index(&mut db, &mut tg, &xqdoc)?;
                        if verbosity > 0 {
                            println!("✅ {}", xqdoc.filename);
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
        let db = Database::new_with_path(db_path, DB_CREATE_OR_OPEN)?;
        let mut iter = IntoIterator::into_iter(tui_app::interactive_query(db)?); // strings is moved here
        while let Some(s) = iter.next() {
            // next() moves a string out of the iter
            println!("{}", s);
        }
    }

    Ok(())
}

fn update_index(
    db: &mut WritableDatabase,
    tg: &mut TermGenerator,
    xqdoc: &XqDocument,
) -> Result<(), Report> {
    // Create a new Xapian Document to store attributes on the passed-in XqDocument
    let mut doc = Document::new()?;
    tg.set_document(&mut doc)?;

    tg.index_text_with_prefix(&xqdoc.author, "A")?;
    tg.index_text_with_prefix(&xqdoc.date_str()?, "D")?;
    tg.index_text_with_prefix(&xqdoc.filename, "F")?;
    tg.index_text_with_prefix(&xqdoc.full_path.clone().into_string().unwrap(), "F")?;
    tg.index_text_with_prefix(&xqdoc.title, "S")?;
    tg.index_text_with_prefix(&xqdoc.subtitle, "XS")?;
    for tag in &xqdoc.tags {
        tg.index_text_with_prefix(&tag, "K")?;
    }

    tg.index_text(&xqdoc.body)?;

    // Convert the XqDocument into JSON and set it in the DB for retrieval later
    doc.set_data(&serde_json::to_string(&xqdoc).unwrap())?;

    let id = "Q".to_owned() + &xqdoc.filename;
    doc.add_boolean_term(&id)?;
    db.replace_document(&id, &mut doc)?;

    Ok(())
}
