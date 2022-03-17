use chrono::{DateTime, FixedOffset};
use color_eyre::Report;
use eyre::{eyre, Result};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::io::{Error, ErrorKind};
use std::{ffi::OsString, fmt, fs, io, marker::PhantomData};
use xapian_rusty::{Document as XapDoc, TermGenerator, WritableDatabase};
use yaml_rust::YamlEmitter;

/// Representation for a given Markdown + FrontMatter file; Example:
/// ---
/// author: Steve Sosik
/// date: 2021-06-22T12:48:16-0400
/// tags:
/// - tika
/// title: This is an example note
/// ---
///
/// Some note here formatted with Markdown syntax
///
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Inherent metadata about the document
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub full_path: OsString,

    /// FrontMatter-derived metadata about the document
    #[serde(default)]
    pub author: String,
    /// RFC 3339 based timestamp
    pub date: String,

    #[serde(deserialize_with = "string_or_list_string")]
    pub tags: Vec<String>,

    pub title: String,

    #[serde(default)]
    pub subtitle: String,

    /// The Markdown-formatted body of the document
    #[serde(default)]
    pub body: String,
}

impl Document {
    pub fn new() -> Self {
        Document {
            filename: String::from(""),
            full_path: OsString::from(""),
            author: String::from(""),
            date: String::from(""),
            tags: vec![],
            title: String::from(""),
            subtitle: String::from(""),
            body: String::from(""),
        }
    }

    pub fn date_str(&self) -> Result<String, Report> {
        if let Ok(t) = self.parse_date() {
            let ret = t.with_timezone(&chrono::Utc).to_rfc3339();
            return Ok(ret);
        }
        Err(eyre!("❌ Failed to convert path to date '{}'", &self.date))
    }
    pub fn parse_date(&self) -> Result<DateTime<FixedOffset>, Report> {
        if let Ok(rfc3339) = DateTime::parse_from_rfc3339(&self.date) {
            return Ok(rfc3339);
        } else if let Ok(s) = DateTime::parse_from_str(&self.date, &String::from("%Y-%m-%dT%T%z")) {
            return Ok(s);
        }
        eprintln!("❌ Failed to convert path to str '{}'", &self.filename);
        Err(eyre!(
            "❌ Failed to convert path to str '{}'",
            &self.filename
        ))
    }

    pub fn update_index(
        &self,
        db: &mut WritableDatabase,
        tg: &mut TermGenerator,
    ) -> Result<(), Report> {
        // Create a new Xapian Document to store attributes on the passed-in Document
        let mut doc = XapDoc::new()?;
        tg.set_document(&mut doc)?;

        tg.index_text_with_prefix(&self.author, "A")?;
        tg.index_text_with_prefix(&self.date_str()?, "D")?;
        tg.index_text_with_prefix(&self.filename, "F")?;
        tg.index_text_with_prefix(&self.full_path.clone().into_string().unwrap(), "F")?;
        tg.index_text_with_prefix(&self.title, "S")?;
        tg.index_text_with_prefix(&self.subtitle, "XS")?;
        for tag in &self.tags {
            tg.index_text_with_prefix(tag, "K")?;
        }

        tg.index_text(&self.body)?;

        // Convert the Document into JSON and set it in the DB for retrieval later
        doc.set_data(&serde_json::to_string(&self).unwrap())?;

        let id = "Q".to_owned() + &self.filename;
        doc.add_boolean_term(&id)?;
        db.replace_document(&id, &mut doc)?;

        Ok(())
    }
}

/// Support Deserializing a string into a list of string of length 1
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

        // Value is a single string: return a Vec containing that single string
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

pub fn parse_file(path: &std::path::Path) -> Result<Document> {
    let full_path = path.to_str().unwrap();
    let s = fs::read_to_string(full_path)?;

    let (yaml, content) = frontmatter::parse_and_find_content(&s).unwrap();
    match yaml {
        Some(yaml) => {
            let mut out_str = String::new();
            {
                let mut emitter = YamlEmitter::new(&mut out_str);
                emitter.dump(&yaml).unwrap(); // dump the YAML object to a String
            }

            let mut doc: Document = serde_yaml::from_str(&out_str)?;
            // TODO Is this check necessary?
            if doc.filename == *"" {
                doc.filename = String::from(path.file_name().unwrap().to_str().unwrap());
            }

            doc.full_path = OsString::from(full_path);

            doc.body = content.to_string();

            Ok(doc)
        }
        None => Err(eyre!("Failed to process file {}", path.display())),
    }
}