use crate::date::{date_deserializer, Date};
use color_eyre::Report;
use eyre::Result;
use serde::{
    de, ser::SerializeSeq, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer,
};
use std::io::{Error, ErrorKind};
use std::str::FromStr;
use std::{ffi::OsString, fmt, fs, io, marker::PhantomData};
use unicode_width::UnicodeWidthStr;
use uuid_b64::UuidB64;
use xapian_rusty::{Document as XapDoc, TermGenerator, WritableDatabase};
use yaml_rust::YamlEmitter;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum SerializationType {
    /// Serialize body only when putting into Storage
    Storage,
    Disk,
    Human,
}

impl Default for SerializationType {
    fn default() -> SerializationType {
        SerializationType::Storage
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct VecString(Vec<String>);

impl VecString {
    pub fn new(v: Vec<String>) -> VecString {
        VecString(v)
    }
}

impl fmt::Display for VecString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join(","))
    }
}

impl Serialize for VecString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for element in &self.0 {
            seq.serialize_element(&element)?;
        }
        seq.end()
    }
}

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
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Document {
    /// Inherent metadata about the document
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub full_path: OsString,

    /// Calculated fields
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    #[serde(skip)]
    pub serialization_type: SerializationType,

    /// FrontMatter-derived metadata about the document
    #[serde(default, alias = "author")]
    pub authors: VecString,

    /// RFC 3339 based timestamp
    /// Epoch seconds
    #[serde(deserialize_with = "date_deserializer")]
    pub date: Date,

    #[serde(default)]
    #[serde(deserialize_with = "string_or_list_string", alias = "tag")]
    pub tags: Vec<String>,

    #[serde(default)]
    pub weight: i32,
    #[serde(default)]
    pub writes: u16,
    #[serde(default)]
    pub views: i32,
    pub title: String,

    #[serde(default)]
    pub subtitle: String,

    /// The Markdown-formatted body of the document
    #[serde(default)]
    pub body: String,
}

#[allow(dead_code)]
fn is_false(v: &bool) -> bool {
    *v
}

impl Document {
    pub fn new() -> Self {
        Document {
            ..Default::default()
        }
    }

    pub fn parse_file(path: &std::path::Path) -> Result<Document, io::Error> {
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

                let mut doc: Document = match serde_yaml::from_str(&out_str) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error reading yaml {}: {:?} {}", full_path, e, out_str);
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("Error reading yaml {}: {}", path.display(), e.to_string()),
                        ));
                    }
                };
                doc.filename = String::from(path.file_name().unwrap().to_str().unwrap());
                doc.body = content.to_string();
                if doc.id.width() == 0 {
                    let uuid = UuidB64::new();
                    doc.id = uuid.to_string();
                }

                Ok(doc)
            }
            None => Err(Error::new(
                ErrorKind::Other,
                format!("Failed to process file {}", path.display()),
            )),
        }
    }

    pub fn update_index(
        &self,
        db: &mut WritableDatabase,
        tg: &mut TermGenerator,
    ) -> Result<(), Report> {
        // Create a new Xapian Document to store attributes on the passed-in Document
        let mut doc = XapDoc::new()?;
        tg.set_document(&mut doc)?;

        tg.index_text_with_prefix(&self.authors.to_string(), "A")?;
        tg.index_text_with_prefix(&self.date.to_string(), "D")?;
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

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.serialization_type == SerializationType::Human {
            write!(f, "{}", self.body)
        } else {
            let yaml = serde_yaml::to_string(&self).unwrap();
            write!(f, "{}---\n{}", yaml, self.body)
        }
    }
}

// Custom Serialization to skip various attributes if requested, ie when writing to disk
impl Serialize for Document {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = match self.serialization_type {
            SerializationType::Storage => serializer.serialize_struct("Document", 14)?,
            SerializationType::Disk => serializer.serialize_struct("Document", 12)?,
            SerializationType::Human => {
                // The Display trait implementation above handles displaying just the
                // document body, don't need to serialize any of the doc metadata
                return serializer.serialize_struct("Document", 0)?.end();
            }
        };

        s.serialize_field("title", &self.title)?;
        if self.subtitle.width() > 0 {
            s.serialize_field("subtitle", &self.subtitle)?;
        };
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("date", &self.date)?;
        } else {
            s.serialize_field("date", &format!("{}", &self.date))?;
        }
        s.serialize_field("tags", &self.tags)?;
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("filename", &self.filename)?;
        };
        s.serialize_field("authors", &self.authors)?;
        s.serialize_field("id", &self.id)?;
        s.serialize_field("weight", &self.weight)?;
        s.serialize_field("writes", &self.writes)?;
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("body", &self.body)?;
        }
        s.end()
    }
}
