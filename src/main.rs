extern crate clap;
use clap::{Arg, App, SubCommand};
use chrono::DateTime;
use frontmatter;
use glob::glob;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{fmt, fs, io, path::Path, env};
use std::marker::PhantomData;
use unwrap::unwrap;
extern crate yaml_rust;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, ReloadPolicy};
//use tempfile::TempDir;
use yaml_rust::YamlEmitter;
extern crate shellexpand;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Doc {
    author: String,
    #[serde(skip_deserializing)]
    body: String,
    date: String,
    #[serde(default)]
    filename: String,
    #[serde(deserialize_with = "string_or_list_string")]
    tags: Vec<String>,
    title: String,
}

fn string_or_list_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
        where
            S: de::SeqAccess<'de>,
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

fn main() -> tantivy::Result<()> {
    color_backtrace::install();

    let default_index_dir = shellexpand::tilde("~/.config/zkfm/");

    let matches = App::new("zkfm")
                      .version("1.0")
                      .author("Steve <steve@little-fluffy.cloud>")
                      .about("Zettlekasten-inspired Markdown+FrontMatter Indexer and query tool")
                      .arg(Arg::with_name("index_path")
                           .short("i")
                           .value_name("DIRECTORY")
                           .help("Set the directory to store Tantivy index data")
                           .default_value(&default_index_dir)
                           .takes_value(true))
                      .arg(Arg::with_name("source")
                           .short("s")
                           .long("source")
                           .value_name("DIRECTORY")
                           .help("Set the source directory containing Markdown docs with Frontmatter")
                           .takes_value(true))
                      .subcommand(SubCommand::with_name("index")
                                  .about("Load data from a source directory")
                                  .arg(Arg::with_name("source")
                                      .help("print debug information verbosely")))
                      .subcommand(SubCommand::with_name("query")
                                  .about("Query the index")
                                  .arg(Arg::with_name("query")
                                      .help("print debug information verbosely")))
                      .get_matches();

    let index_path = matches.value_of("index_path").unwrap();

    let index_path = Path::new(&index_path);
    fs::create_dir_all(index_path)?;

    let mut schema_builder = Schema::builder();
    let author = schema_builder.add_text_field("author", TEXT);
    let body = schema_builder.add_text_field("body", TEXT);
    let date = schema_builder.add_date_field("date", INDEXED | STORED);
    let filename = schema_builder.add_text_field("filename", TEXT | STORED);
    let tags = schema_builder.add_text_field("tags", TEXT | STORED);
    let title = schema_builder.add_text_field("title", TEXT | STORED);

    let schema = schema_builder.build();

    let d = tantivy::directory::MmapDirectory::open(index_path).unwrap();
    let index = Index::open_or_create(d, schema.clone()).unwrap();

    if let Some(matches) = matches.subcommand_matches("index") {
        let source = matches.value_of("source").unwrap();

        let mut index_writer = index.writer(100_000_000).unwrap();

        let glob_path = Path::new(&source).join("*.md");
        let glob_str = glob_path.to_str().unwrap();

        println!("Directory: {}", glob_str);

        for entry in glob(glob_str).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    println!("Processing {:?}", path.display());
                    let res = index_file(&path);
                    let doc = unwrap!(res, "Failed to process file {}", path.display());
                    let rfc3339 = DateTime::parse_from_rfc3339(&doc.date).unwrap();
                    let thingit = rfc3339.with_timezone(&chrono::Utc);
                    let thedate = Value::Date(thingit);

                    index_writer.add_document(doc!(
                        author => doc.author,
                        body => doc.body,
                        date => thedate,
                        filename => doc.filename,
                        tags => doc.tags.join(" "),
                        title => doc.title,
                    ));
                }
                Err(e) => println!("{:?}", e),
            }
        }

        index_writer.commit().unwrap();
    }

    if let Some(matches) = matches.subcommand_matches("query") {
        let query = matches.value_of("query").unwrap();

        // TODO replace with meaningful queries
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;

        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(&index, vec![author, body, filename, tags, title]);

        //let query = query_parser.parse_query("vim")?;
        //let query = query_parser.parse_query("tags:kubernetes")?;
        //let query = query_parser.parse_query("date:2020-07-24T13:03:50-04:00")?;
        //let query = query_parser.parse_query("* AND date:\"2019-04-01T14:02:03Z\"")?;
        //let query = query_parser.parse_query("* AND NOT date:\"2019-04-01T14:02:03Z\"")?;
        let query = query_parser.parse_query(&query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(100))?;

        for (_score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc(doc_address)?;
            println!("{}", schema.to_json(&retrieved_doc));
        }
    }

    //println!("index_path: {:?}", index_path);
    Ok(())
}

fn index_file(path: &std::path::PathBuf) -> Result<Doc, io::Error> {
    let s = fs::read_to_string(path.to_str().unwrap())?;

    let (yaml, content) = frontmatter::parse_and_find_content(&s).unwrap();
    let yaml = yaml.unwrap();

    let mut out_str = String::new();
    {
        let mut emitter = YamlEmitter::new(&mut out_str);
        emitter.dump(&yaml).unwrap(); // dump the YAML object to a String
    }

    let mut doc: Doc = serde_yaml::from_str(&out_str).unwrap();
    if doc.filename == "".to_string() {
        doc.filename = String::from(path.file_name().unwrap().to_str().unwrap());
    }

    doc.body = content.to_string();

    //println!("doc {:?}", doc);

    Ok(doc)
}
