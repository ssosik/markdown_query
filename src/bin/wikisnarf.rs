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
use xapiary::xq_document::XqDocument;

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

struct Parser {
    state: ParserState,
    title_complete: bool,
    username_complete: bool,
    timestamp_complete: bool,
    text_complete: bool,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            state: ParserState::Between,
            title_complete: false,
            username_complete: false,
            timestamp_complete: false,
            text_complete: false,
        }
    }

    fn reset_complete(&mut self) {
        self.title_complete = false;
        self.username_complete = false;
        self.timestamp_complete = false;
        self.text_complete = false;
    }

    fn record_is_complete(&mut self) -> bool {
        self.title_complete
            && self.username_complete
            && self.timestamp_complete
            && self.text_complete
    }

    pub fn process(&mut self, ev: Event, xqdoc: &mut XqDocument) -> Result<bool, Box<dyn Error>> {
        self.state = match self.state {
            ParserState::Between => match ev {
                Event::Start(e) if e.local_name() == b"page" => ParserState::ReadingPage,
                _ => ParserState::Between,
            },

            ParserState::ReadingPage => match ev {
                Event::End(e) if e.local_name() == b"page" => ParserState::Between,
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
                    xqdoc.title = String::from(str::from_utf8(&e.unescaped()?)?);
                    xqdoc.filename = String::from(str::from_utf8(&e.unescaped()?)?);
                    self.title_complete = true;
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad title text in {:?}", ev);
                    return Err("Bad title text".into());
                }
            },

            ParserState::ReadingTimestamp => match ev {
                Event::Text(e) => {
                    xqdoc.date = String::from(str::from_utf8(&e.unescaped()?)?);
                    self.username_complete = true;
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad date text in {:?}", ev);
                    return Err("Bad date text".into());
                }
            },

            ParserState::ReadingUsername => match ev {
                Event::Text(e) => {
                    xqdoc.author = String::from(str::from_utf8(&e.unescaped()?)?);
                    self.timestamp_complete = true;
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad author text in {:?}", ev);
                    return Err("Bad author text".into());
                }
            },

            ParserState::ReadingText => match ev {
                Event::Text(e) => {
                    xqdoc.body = String::from(str::from_utf8(&e.unescaped()?)?);
                    self.text_complete = true;
                    ParserState::ReadingPage
                }
                _ => {
                    eprintln!("Bad body text in {:?}", ev);
                    return Err("Bad body text".into());
                }
            },
        };

        Ok(self.record_is_complete())
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

    let mut parser = Parser::new();
    let mut doc = XqDocument::new();
    doc.tags = vec![String::from("wikipedia")];

    let mut db = WritableDatabase::new(dbpath.as_str(), BRASS, DB_CREATE_OR_OPEN)?;
    let mut tg = TermGenerator::new()?;
    let mut stemmer = Stem::new("en")?;
    tg.set_stemmer(&mut stemmer)?;

    loop {
        if match xmlfile.read_event(&mut buf)? {
            Event::Eof => break,
            ev => parser.process(ev, &mut doc)?,
        } {
            doc.update_index(&mut db, &mut tg)?;
            bar.inc(BUF_SIZE as u64);
            parser.reset_complete();
            doc = XqDocument::new();
            doc.tags = vec![String::from("wikipedia")];
        }
        buf.clear();
    }
    bar.finish();

    Ok(())
}
