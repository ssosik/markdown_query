// XQ utility for loading in a compressed Wikipedia backup and indexing the data
// To be used for testing xq querying.
// XML Parsing idea from https://usethe.computer/posts/14-xmhell.html

use bzip2::bufread::MultiBzDecoder;
use color_eyre::Report;
use encoding_rs_io::DecodeReaderBytes;
use indicatif::{ProgressBar, ProgressStyle};
use quick_xml::{events::Event, Reader};
use std::fs;
use std::{env, error::Error, io::BufReader, str};
use xapian_rusty::{Stem, TermGenerator, WritableDatabase, BRASS, DB_CREATE_OR_OPEN};
use mdq::xq_document::XqDocument;

const BUF_SIZE: usize = 4096 * 8; // 32kb at once

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}

// Wikipedia document looks like
//   <page>
//    <title>AccessibleComputing</title>
//    <ns>0</ns>
//    <id>10</id>
//    <redirect title="Computer accessibility" />
//    <revision>
//      <id>1002250816</id>
//      <parentid>854851586</parentid>
//      <timestamp>2021-01-23T15:15:01Z</timestamp>
//      <contributor>
//        <username>Elli</username>
//        <id>20842734</id>
//      </contributor>
//      <minor />
//      <comment>shel</comment>
//      <model>wikitext</model>
//      <format>text/x-wiki</format>
//      <text bytes="111" xml:space="preserve">#REDIRECT [[Computer accessibility]]
//
//{{rcat shell|
//{{R from move}}
//{{R from CamelCase}}
//{{R unprintworthy}}
//}}</text>
//      <sha1>kmysdltgexdwkv2xsml3j44jb56dxvn</sha1>
//    </revision>
//  </page>

#[derive(Copy, Clone, Debug)]
enum ParserState {
    Between,
    ReadingPage,
    ReadingTitle,
    ReadingTimestamp,
    ReadingUsername,
    ReadingText,
}

struct Parser<'a> {
    state: ParserState,
    xqdoc: XqDocument,
    db: &'a mut WritableDatabase,
    tg: &'a mut TermGenerator,
}

impl<'b> Parser<'b> {
    pub fn new(db: &'b mut WritableDatabase, tg: &'b mut TermGenerator) -> Self {
        let mut xqdoc = XqDocument::new();
        xqdoc.tags = vec![String::from("wikipedia")];
        Parser {
            state: ParserState::Between,
            xqdoc,
            db,
            tg,
        }
    }

    pub fn process(&mut self, ev: Event) -> Result<(), Box<dyn Error>> {
        self.state = match self.state {
            ParserState::Between => match ev {
                Event::Start(e) if e.local_name() == b"page" => {
                    // New Doc to index
                    let mut doc = XqDocument::new();
                    doc.tags = vec![String::from("wikipedia")];
                    self.xqdoc = doc;
                    ParserState::ReadingPage
                }
                _ => ParserState::Between,
            },

            ParserState::ReadingPage => match ev {
                Event::End(e) if e.local_name() == b"page" => {
                    // Publish completed record
                    self.xqdoc.update_index(&mut self.db, &mut self.tg)?;
                    ParserState::Between
                }
                Event::Start(e) => match e.local_name() {
                    b"title" => ParserState::ReadingTitle,
                    b"username" => ParserState::ReadingUsername,
                    b"timestamp" => ParserState::ReadingTimestamp,
                    b"text" => ParserState::ReadingText,
                    _ => {
                        // Current XML tag is something we're not interested in, skip
                        ParserState::ReadingPage
                    }
                },

                _ => {
                    // Currently not parsing a `page` record, skip
                    ParserState::ReadingPage
                }
            },

            ParserState::ReadingTitle => match ev {
                Event::Text(e) => {
                    self.xqdoc.title = String::from(str::from_utf8(&e.unescaped()?)?);
                    self.xqdoc.filename = String::from(str::from_utf8(&e.unescaped()?)?);
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad title text in {:?}", ev);
                    return Err("Bad title text".into());
                }
            },

            ParserState::ReadingTimestamp => match ev {
                Event::Text(e) => {
                    self.xqdoc.date = String::from(str::from_utf8(&e.unescaped()?)?);
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad date text in {:?}", ev);
                    return Err("Bad date text".into());
                }
            },

            ParserState::ReadingUsername => match ev {
                Event::Text(e) => {
                    self.xqdoc.author = String::from(str::from_utf8(&e.unescaped()?)?);
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad author text in {:?}", ev);
                    return Err("Bad author text".into());
                }
            },

            ParserState::ReadingText => match ev {
                Event::Text(e) => {
                    self.xqdoc.body = String::from(str::from_utf8(&e.unescaped()?)?);
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad body text in {:?}", ev);
                    return Err("Bad body text".into());
                }
            },
        };

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    setup()?;

    let dbpath = env::args().nth(1).ok_or("no db path provided")?;
    let mut buf = Vec::with_capacity(BUF_SIZE);

    let path = env::args().nth(2).ok_or("no zipfile")?;

    let metadata = fs::metadata(&path)?;
    // Wrong! This is the compressed file size, not the uncompressed file size
    let bar = ProgressBar::new(metadata.len());
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} Processed {bytes}/{total_bytes} {percent:>4}% @ {bytes_per_sec} {eta_precise} remaining {msg}"),
    );

    let zipfile = fs::File::open(path)?;
    let reader = BufReader::new(zipfile);
    let reader = MultiBzDecoder::new(reader);
    let reader = BufReader::new(DecodeReaderBytes::new(reader));
    let mut xmlfile = Reader::from_reader(reader);

    let mut db = WritableDatabase::new(dbpath.as_str(), BRASS, DB_CREATE_OR_OPEN)?;
    let mut tg = TermGenerator::new()?;
    let mut stemmer = Stem::new("en")?;
    tg.set_stemmer(&mut stemmer)?;

    let mut parser = Parser::new(&mut db, &mut tg);
    loop {
        match xmlfile.read_event(&mut buf)? {
            Event::Eof => break,
            ev => parser.process(ev)?,
        }
        bar.inc(BUF_SIZE as u64);
        buf.clear();
    }
    bar.finish();

    Ok(())
}
