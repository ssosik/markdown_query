use crate::tika_document::TikaDocument;
use color_eyre::Report;
use eyre::{eyre, Result};
#[allow(unused)]
use nom::{
    bytes::streaming::{is_not, tag, tag_no_case, take_until},
    character::complete::multispace1 as complete_multispace1,
    character::streaming::{alphanumeric0, alphanumeric1, multispace0, multispace1, space0},
    combinator::{recognize, value},
    multi::{many0, many1},
    sequence::{delimited, pair, separated_pair, tuple},
    {alt, branch::alt, complete, delimited, named, tag, take_until, value}, // {IResult},
};
use std::convert::From;
use std::fmt;
use std::str;
use xapian_rusty::FeatureFlag::{
    FlagBoolean, FlagBooleanAnyCase, FlagLovehate, FlagPartial, FlagPhrase, FlagPureNot,
    FlagSpellingCorrection, FlagWildcard,
};
use xapian_rusty::{Database, Query, QueryParser, Stem, XapianOp, DB_CREATE_OR_OVERWRITE};

// The most helpful write-up on using Nom that I've seen so far:
//   https://iximiuz.com/en/posts/rust-writing-parsers-with-nom/

// Local representation of xapian expression operators, most notably these are Copy!
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MatchOp {
    And,
    AndNot,
    Or,
    Xor,
    AndMaybe,
    Filter,
    Near,
    Phrase,
    ValueRange,
    ScaleWeight,
    EliteSet,
    ValueGe,
    ValueLe,
    Synonym,
}

// Support converting into and from XapianOps
impl From<MatchOp> for XapianOp {
    fn from(item: MatchOp) -> Self {
        match item {
            MatchOp::And => XapianOp::OpAnd,
            MatchOp::AndNot => XapianOp::OpAndNot,
            MatchOp::Or => XapianOp::OpOr,
            MatchOp::Xor => XapianOp::OpXor,
            MatchOp::AndMaybe => XapianOp::OpAndMaybe,
            MatchOp::Filter => XapianOp::OpFilter,
            MatchOp::Near => XapianOp::OpNear,
            MatchOp::Phrase => XapianOp::OpPhrase,
            MatchOp::ValueRange => XapianOp::OpValueRange,
            MatchOp::ScaleWeight => XapianOp::OpScaleWeight,
            MatchOp::EliteSet => XapianOp::OpEliteSet,
            MatchOp::ValueGe => XapianOp::OpValueGe,
            MatchOp::ValueLe => XapianOp::OpValueLe,
            MatchOp::Synonym => XapianOp::OpSynonym,
        }
    }
}

impl From<XapianOp> for MatchOp {
    fn from(item: XapianOp) -> Self {
        match item {
            XapianOp::OpAnd => MatchOp::And,
            XapianOp::OpAndNot => MatchOp::AndNot,
            XapianOp::OpOr => MatchOp::Or,
            XapianOp::OpXor => MatchOp::Xor,
            XapianOp::OpAndMaybe => MatchOp::AndMaybe,
            XapianOp::OpFilter => MatchOp::Filter,
            XapianOp::OpNear => MatchOp::Near,
            XapianOp::OpPhrase => MatchOp::Phrase,
            XapianOp::OpValueRange => MatchOp::ValueRange,
            XapianOp::OpScaleWeight => MatchOp::ScaleWeight,
            XapianOp::OpEliteSet => MatchOp::EliteSet,
            XapianOp::OpValueGe => MatchOp::ValueGe,
            XapianOp::OpValueLe => MatchOp::ValueLe,
            XapianOp::OpSynonym => MatchOp::Synonym,
        }
    }
}

impl fmt::Display for MatchOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MatchOp::And => write!(f, "<And>"),
            MatchOp::AndNot => write!(f, "<AndNot>"),
            MatchOp::Or => write!(f, "<Or>"),
            MatchOp::Xor => write!(f, "<Xor>"),
            MatchOp::AndMaybe => write!(f, "<AndMaybe>"),
            MatchOp::Filter => write!(f, "<Filter>"),
            MatchOp::Near => write!(f, "<Near>"),
            MatchOp::Phrase => write!(f, "<Phrase>"),
            MatchOp::ValueRange => write!(f, "<ValueRange>"),
            MatchOp::ScaleWeight => write!(f, "<ScaleWeight>"),
            MatchOp::EliteSet => write!(f, "<EliteSet>"),
            MatchOp::ValueGe => write!(f, "<ValueGe>"),
            MatchOp::ValueLe => write!(f, "<ValueLe>"),
            MatchOp::Synonym => write!(f, "<Synonym>"),
        }
    }
}

pub fn matchop(input: &str) -> IResult<MatchOp> {
    alt((
        value(MatchOp::AndMaybe, tag_no_case("AND MAYBE")),
        value(MatchOp::Synonym, tag_no_case("SYNONYM")),
        value(MatchOp::AndNot, tag_no_case("AND NOT")),
        value(MatchOp::Filter, tag_no_case("FILTER")),
        value(MatchOp::Phrase, tag_no_case("PHRASE")),
        value(MatchOp::ScaleWeight, tag_no_case("SCALED")),
        value(MatchOp::ValueRange, tag_no_case("RANGE")),
        value(MatchOp::EliteSet, tag_no_case("ELITE")),
        value(MatchOp::Near, tag_no_case("NEAR")),
        value(MatchOp::And, tag_no_case("AND")),
        value(MatchOp::Xor, tag_no_case("XOR")),
        value(MatchOp::Or, tag_no_case("OR")),
        value(MatchOp::ValueGe, tag_no_case(">")),
        value(MatchOp::ValueLe, tag_no_case("<")),
    ))(Span::new(input))
}

#[cfg(test)]
mod matchop_tests {
    use super::*;
    #[test]
    fn test1() {
        let (rest, op) = matchop("AND\n").expect("Failed to match Op");
    }
}

use nom_locate::LocatedSpan;

pub type Span<'a> = LocatedSpan<&'a str>;

pub type IResult<'a, O> = nom::IResult<Span<'a>, O>;

#[allow(dead_code)]
fn word(input: Span) -> IResult<Span> {
    // TODO should more characters be supported in a "word"?
    // Use `recognize` here to discard the actual parsed value and return the matched substring as
    // a result
    recognize(many1(alt((recognize(alphanumeric1), recognize(tag("_"))))))(input)
}

#[allow(dead_code)]
struct ExpectedParseResult<'a> {
    matched_fragment: &'a str,
    matched_offset: usize,
    matched_line: u32,
    matched_column: usize,
    rest_fragment: &'a str,
    rest_offset: usize,
    rest_line: u32,
    rest_column: usize,
}

#[allow(dead_code)]
impl ExpectedParseResult<'_> {
    fn new<'a>(
        mf: &'a str,
        mo: usize,
        ml: u32,
        mc: usize,
        rf: &'a str,
        ro: usize,
        rl: u32,
        rc: usize,
    ) -> ExpectedParseResult<'a> {
        ExpectedParseResult {
            matched_fragment: mf,
            matched_offset: mo,
            matched_line: ml,
            matched_column: mc,
            rest_fragment: rf,
            rest_offset: ro,
            rest_line: rl,
            rest_column: rc,
        }
    }
    fn compare(self, f: &dyn Fn(Span) -> IResult<Span>, s: &str) {
        let (rest, matched) = f(Span::new(s)).expect("Failed to parse input");

        assert_eq!(&self.matched_fragment, matched.fragment());
        assert_eq!(self.matched_offset, matched.location_offset());
        assert_eq!(self.matched_line, matched.location_line());
        assert_eq!(self.matched_column, matched.get_column());

        assert_eq!(&self.rest_fragment, rest.fragment());
        assert_eq!(self.rest_offset, rest.location_offset());
        assert_eq!(self.rest_line, rest.location_line());
        assert_eq!(self.rest_column, rest.get_column());
    }
}

#[cfg(test)]
mod word_tests {
    use super::*;
    #[test]
    fn one_word_no_trailing_space() {
        assert!(word(Span::new(r#"foo"#)).is_err())
    }

    #[test]
    fn one_word_with_trailing_space() {
        ExpectedParseResult::new(&"foo", 0, 1, 1, &" ", 3, 1, 4).compare(&word, &r#"foo "#)
    }

    #[test]
    fn one_word_with_trailing_newline() {
        ExpectedParseResult::new(&"foo", 0, 1, 1, &"\\n", 3, 1, 4).compare(&word, &r#"foo\n"#)
    }

    #[test]
    fn two_space_separated_words() {
        ExpectedParseResult::new(&"foo", 0, 1, 1, &" bar", 3, 1, 4).compare(&word, &r#"foo bar"#)
    }
}

fn words(input: Span) -> IResult<Span> {
    recognize(many1(alt((recognize(multispace1), recognize(word)))))(input)
}

#[cfg(test)]
mod words_tests {
    use super::*;
    #[test]
    fn one_word_no_trailing_newline() {
        assert!(words(Span::new(r#"foo"#)).is_err())
    }

    #[test]
    fn one_word() {
        ExpectedParseResult::new(&"foo", 0, 1, 1, &"\\n", 3, 1, 4).compare(&words, &r#"foo\n"#)
    }

    #[test]
    fn two_space_separated_words() {
        ExpectedParseResult::new(&"foo bar", 0, 1, 1, &"\\n", 7, 1, 8)
            .compare(&words, &r#"foo bar\n"#)
    }
}

fn quoted(input: Span) -> IResult<Span> {
    recognize(alt((
        delimited(
            recognize(tag(r#"""#)),
            recognize(words),
            recognize(tag(r#"""#)),
        ),
        delimited(
            recognize(tag(r#"'"#)),
            recognize(words),
            recognize(tag(r#"'"#)),
        ),
    )))(input)
}

#[cfg(test)]
mod quoted_tests {
    use super::*;
    #[test]
    fn one_word_no_trailing_space() {
        ExpectedParseResult::new(&"\"foo\"", 0, 1, 1, &"", 5, 1, 6).compare(&quoted, &r#""foo""#)
    }

    #[test]
    fn one_word_with_trailing_space() {
        ExpectedParseResult::new(&"\"foo \"", 0, 1, 1, &"", 6, 1, 7).compare(&quoted, &r#""foo ""#)
    }

    #[test]
    fn two_words() {
        ExpectedParseResult::new(&"\"foo bar\"", 0, 1, 1, &"", 9, 1, 10)
            .compare(&quoted, &r#""foo bar""#)
    }

    #[test]
    fn single_quote_one_word_no_trailing_space() {
        ExpectedParseResult::new(&"\'foo\'", 0, 1, 1, &"", 5, 1, 6).compare(&quoted, &r#"'foo'"#)
    }

    #[test]
    fn single_quote_one_word_with_trailing_space() {
        ExpectedParseResult::new(&"\'foo \'", 0, 1, 1, &"", 6, 1, 7).compare(&quoted, &r#"'foo '"#)
    }

    #[test]
    fn single_quote_two_words() {
        ExpectedParseResult::new(&"\'foo bar\'", 0, 1, 1, &"", 9, 1, 10)
            .compare(&quoted, &r#"'foo bar'"#)
    }

    #[test]
    fn tag_entirely_single_quoted() {
        // The colon character currently isn't an allowed `word` character
        assert!(tagged(Span::new(r#"'foo:bar'"#)).is_err())
    }

    #[test]
    fn tag_entirely_double_quoted() {
        // The colon character currently isn't an allowed `word` character
        assert!(tagged(Span::new(r#""foo:bar""#)).is_err())
    }
}

fn tagged(input: Span) -> IResult<Span> {
    recognize(tuple((word, tag(":"), alt((quoted, word)), multispace0)))(input)
}

#[cfg(test)]
mod tagged_tests {
    use super::*;
    #[test]
    fn one_word_no_trailing_space() {
        assert!(tagged(Span::new(r#"foo:bar"#)).is_err())
    }

    #[test]
    fn one_word_with_trailing_space() {
        ExpectedParseResult::new(&"foo:bar", 0, 1, 1, &"\\n", 7, 1, 8)
            .compare(&tagged, &r#"foo:bar\n"#)
    }

    #[test]
    fn two_words() {
        ExpectedParseResult::new(&"tag:foo ", 0, 1, 1, &"bar", 8, 1, 9)
            .compare(&tagged, &r#"tag:foo bar"#)
    }

    #[test]
    fn two_words_single_quoted() {
        ExpectedParseResult::new(&"tag:\'foo bar\'", 0, 1, 1, &"\\n", 13, 1, 14)
            .compare(&tagged, &r#"tag:'foo bar'\n"#)
    }

    #[test]
    fn two_words_double_quoted() {
        ExpectedParseResult::new(&"tag:\"foo bar\"", 0, 1, 1, &"\\n", 13, 1, 14)
            .compare(&tagged, &r#"tag:"foo bar"\n"#)
    }

    #[test]
    fn tag_entirely_single_quoted() {
        assert!(tagged(Span::new(r#"'foo:bar'"#)).is_err())
    }

    #[test]
    fn tag_entirely_double_quoted() {
        assert!(tagged(Span::new(r#""foo:bar""#)).is_err())
    }
}

// Xapian tags in human format, e.g. "author:" or "title:"
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XapianTag {
    Author,
    Date,
    Filename,
    Fullpath,
    Title,
    Subtitle,
    Tag,
}

impl XapianTag {
    fn to_xapian<'a>(self) -> &'a str {
        match self {
            XapianTag::Author => "A",
            XapianTag::Date => "D",
            XapianTag::Filename => "F",
            XapianTag::Fullpath => "F",
            XapianTag::Title => "S",
            XapianTag::Subtitle => "XS",
            XapianTag::Tag => "K",
        }
    }
    pub fn parse(input: Span) -> IResult<(XapianTag, Span)> {
        separated_pair(
            alt((
                value(XapianTag::Filename, tag_no_case("filename")),
                value(XapianTag::Fullpath, tag_no_case("fullpath")),
                value(XapianTag::Subtitle, tag_no_case("subtitle")),
                value(XapianTag::Author, tag_no_case("author")),
                value(XapianTag::Title, tag_no_case("title")),
                value(XapianTag::Date, tag_no_case("date")),
                value(XapianTag::Tag, tag_no_case("tag")),
            )),
            tag(":"),
            alt((quoted, word)),
        )(input)
    }
}

impl fmt::Display for XapianTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{}>", self.to_xapian())
    }
}

mod xapiantag_tests {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn unrecognized_tag() {
        assert!(XapianTag::parse(Span::new(r#"foo:bar"#)).is_err())
    }

    #[test]
    fn tag_no_trailing_whitespace() {
        assert!(XapianTag::parse(Span::new(r#"author:bar"#)).is_err())
    }

    #[test]
    fn one_word_tag() {
        let (rest, (tag, value)) =
            XapianTag::parse(Span::new(r#"author:bar "#)).expect("Failed to parse input");
        assert_eq!("A", tag.to_xapian());
        assert_eq!(&"bar", value.fragment());
        assert_eq!(&" ", rest.fragment());
    }

    #[test]
    fn two_word_tag() {
        let (rest, (tag, value)) =
            XapianTag::parse(Span::new(r#"author:bar other"#)).expect("Failed to parse input");
        assert_eq!("A", tag.to_xapian());
        assert_eq!(&"bar", value.fragment());
        assert_eq!(&" other", rest.fragment());
    }
}

fn expression(input: Span) -> IResult<Vec<Span>> {
    many1(alt((quoted, tagged, word, multispace1)))(input)
}

fn whitespace(input: Span) -> IResult<Span> {
    recognize(many1(complete_multispace1))(input)
}

fn span_into_query(qp: &mut QueryParser, flags: i16, token: Span) -> Result<Query, Report> {
    match XapianTag::parse(token) {
        Ok((_rest, (tag, value))) => {
            //println!("TAG: {} {} {}", tag.to_xapian(), value, _rest);
            Ok(qp.parse_query_with_prefix(&value, flags, tag.to_xapian())?)
        }
        Err(_e) => {
            //println!("Span: {} Error: {}", token, e);
            Ok(qp.parse_query(*token, flags)?)
        }
    }
}

fn expression_into_query(mut qp: QueryParser, flags: i16, qstr: &str) -> Result<Query, Report> {
    // Parse the query string into a Vec of matches
    let mut matches = match expression(Span::new(qstr)) {
        Ok((_rest, matches)) => matches.into_iter(),
        Err(_) => return Ok(qp.parse_query("", flags)?),
    };

    let token = matches.next();
    if token.is_none() {
        return Err(eyre!("Empty expression"));
    }

    let mut query = span_into_query(&mut qp, flags, token.unwrap())?;

    for token in matches {
        // Skip whitespace-only tokens
        if let Ok(_) = whitespace(token) {
            continue;
        }

        query = query.add_right(XapianOp::OpOr, &mut span_into_query(&mut qp, flags, token)?)?;
    }

    Ok(query)
}

#[cfg(test)]
mod expression_tests {
    use super::*;
    #[test]
    fn example1() {
        let mut qp = QueryParser::new().expect("Failed to create queryparser");
        let mut stem = Stem::new("en").expect("Failed to create stemmer");
        qp.set_stemmer(&mut stem).expect("Failed to set stemmer");

        let flags = FlagBoolean as i16
            | FlagPhrase as i16
            | FlagLovehate as i16
            | FlagBooleanAnyCase as i16
            | FlagWildcard as i16
            | FlagPureNot as i16
            | FlagPartial as i16
            | FlagSpellingCorrection as i16;

        let s = &r#"title:foo  baz bar author:bob hee tag:rust "hee hee hee" \n"#;
        let mut query = expression_into_query(qp, flags, s).expect("Failed to parse");
        assert_eq!("Query((((((((WILDCARD SYNONYM Sfoo OR ZSfoo@1) OR (WILDCARD SYNONYM baz OR Zbaz@1)) OR (WILDCARD SYNONYM bar OR Zbar@1)) OR (WILDCARD SYNONYM Abob OR ZAbob@1)) OR (WILDCARD SYNONYM hee OR Zhee@1)) OR (WILDCARD SYNONYM Krust OR ZKrust@1)) OR (hee@1 PHRASE 3 hee@2 PHRASE 3 hee@3)))",
        query.get_description(),
        "Generated query didn't match expected for input string '{}'", s);
    }

    #[test]
    fn example2() {
        let mut qp = QueryParser::new().expect("Failed to create queryparser");
        let mut stem = Stem::new("en").expect("Failed to create stemmer");
        qp.set_stemmer(&mut stem).expect("Failed to set stemmer");

        let flags = FlagBoolean as i16
            | FlagPhrase as i16
            | FlagLovehate as i16
            | FlagBooleanAnyCase as i16
            | FlagWildcard as i16
            | FlagPureNot as i16
            | FlagPartial as i16
            | FlagSpellingCorrection as i16;

        let s = &r#"title:"foo bar" author:bob tag:rust\n"#;
        let mut query = expression_into_query(qp, flags, s).expect("Failed to parse");
        assert_eq!("Query((((Sfoo@1 PHRASE 2 Sbar@2) OR (WILDCARD SYNONYM Abob OR ZAbob@1)) OR (tag@1 PHRASE 2 rust@2)))",
        query.get_description(),
        "Generated query didn't match expected for input string '{}'", s);
    }

    #[test]
    fn example3() {
        let mut qp = QueryParser::new().expect("Failed to create queryparser");
        let mut stem = Stem::new("en").expect("Failed to create stemmer");
        qp.set_stemmer(&mut stem).expect("Failed to set stemmer");

        let flags = FlagBoolean as i16
            | FlagPhrase as i16
            | FlagLovehate as i16
            | FlagBooleanAnyCase as i16
            | FlagWildcard as i16
            | FlagPureNot as i16
            | FlagPartial as i16
            | FlagSpellingCorrection as i16;

        let s = &r#"title:foo "baz bar" author:"bob alice" hee tag:rust "hee hee"\n"#;
        let mut query = expression_into_query(qp, flags, s).expect("Failed to parse");
        assert_eq!("Query(((((((WILDCARD SYNONYM Sfoo OR ZSfoo@1) OR (baz@1 PHRASE 2 bar@2)) OR (Abob@1 PHRASE 2 Aalice@2)) OR (WILDCARD SYNONYM hee OR Zhee@1)) OR (WILDCARD SYNONYM Krust OR ZKrust@1)) OR (hee@1 PHRASE 2 hee@2)))",
        query.get_description(),
        "Generated query didn't match expected for input string '{}'", s);
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;
    #[test]
    #[ignore] // TODO figure out why this fails
    fn test1() {
        let query_str = r#"eep op tag:meh fooobarr AND maybe maybe foo AND bar\n"#;
        let mut result = parse_user_query(query_str).expect("Failed to parse");
        assert_eq!(
            "Query((((Zeep@1 OR Zop@2 OR (tag@3 PHRASE 2 meh@4) OR Zfooobarr@5) AND_MAYBE (Zmayb@1 OR Zfoo@2)) AND (bar@1 PHRASE 2 n@2)))",
            //"Query(((((eep@1 PHRASE 2 op@2) OR (tag@3 PHRASE 2 meh@4) OR Zfooobarr@5) AND_MAYBE (Zmayb@1 OR Zfoo@2)) AND (bar@1 PHRASE 2 n@2)))",
            result.get_description()
        );
    }

    #[test]
    #[ignore] // TODO figure out why this fails
    fn test2() {
        let query_str = r#""eep op" tag:meh fooobarr AND maybe maybe foo AND bar\n"#;
        let mut result = parse_user_query(query_str).expect("Failed to parse");
        assert_eq!(
            "Query(((((eep@1 PHRASE 2 op@2) OR (tag@3 PHRASE 2 meh@4) OR Zfooobarr@5) AND_MAYBE (Zmayb@1 OR Zfoo@2)) AND (bar@1 PHRASE 2 n@2)))", 
            result.get_description()
        );
    }
}

pub fn parse_user_query(mut qstr: &str) -> Result<Query, Report> {
    let mut qp = QueryParser::new()?;
    let mut stem = Stem::new("en")?;
    qp.set_stemmer(&mut stem)?;

    let flags = FlagBoolean as i16
        | FlagPhrase as i16
        | FlagLovehate as i16
        | FlagBooleanAnyCase as i16
        | FlagWildcard as i16
        | FlagPureNot as i16
        | FlagPartial as i16
        | FlagSpellingCorrection as i16;

    let mut query;
    let mut operator;

    // Create the initial query
    match take_up_to_operator(qstr.as_bytes()) {
        Ok((rest, matched)) => {
            query = qp.parse_query(str::from_utf8(matched)?, flags)?;
            qstr = str::from_utf8(rest)?;
        }
        Err(_) => {
            // No operator found in the initial string, return a query for the entire string
            return expression_into_query(qp, flags, qstr);
        }
    }

    // Pop off the operator and store it for the next 'add_right' call
    if let Ok((rest, op)) = matchop(qstr) {
        operator = op;
        qstr = *rest;
    } else {
        // This shouldn't ever happen
        panic!("Couldn't match leading operator in {}", qstr);
    }

    let mut depth = 0;
    while qstr.len() > 0 {
        depth += 1;

        // Take the next chunk up to the next operator and add it to the query
        match take_up_to_operator(qstr.as_bytes()) {
            Ok((rest, matched)) => {
                query = query.add_right(
                    operator.into(),
                    &mut qp.parse_query(str::from_utf8(matched)?, flags)?,
                )?;
                qstr = str::from_utf8(rest)?;
            }
            Err(_e) => {
                // There are no more operators, parse the rest of the string into a query and break
                query = query.add_right(
                    operator.into(),
                    &mut expression_into_query(qp, flags, qstr)?,
                )?;
                // No more operators found, break out of the loop
                break;
            }
        }

        // Pop off the operator and store it for the next 'add_right' call
        if let Ok((rest, op)) = matchop(qstr) {
            operator = op;
            qstr = *rest;
        } else {
            // This shouldn't ever happen
            panic!("Couldn't match leading operator in {}", qstr);
        }

        if depth > 50 {
            panic!("Depth limit reached with remaining '{}'", qstr);
        }
    }

    Ok(query)
}

// TODO is there a better way to handle case insensitity here?
named!(
    take_up_to_operator,
    alt!(
        complete!(take_until!("AND MAYBE"))
            | complete!(take_until!("and maybe"))
            | complete!(take_until!("AND NOT"))
            | complete!(take_until!("and not"))
            | complete!(take_until!("SYNONYM"))
            | complete!(take_until!("synonym"))
            | complete!(take_until!("FILTER"))
            | complete!(take_until!("filter"))
            | complete!(take_until!("PHRASE"))
            | complete!(take_until!("phrase"))
            | complete!(take_until!("SCALED"))
            | complete!(take_until!("scaled"))
            | complete!(take_until!("ELITE"))
            | complete!(take_until!("elite"))
            | complete!(take_until!("RANGE"))
            | complete!(take_until!("range"))
            | complete!(take_until!("NEAR"))
            | complete!(take_until!("near"))
            | complete!(take_until!("AND"))
            | complete!(take_until!("and"))
            | complete!(take_until!("XOR"))
            | complete!(take_until!("xor"))
            | complete!(take_until!("OR"))
            | complete!(take_until!("or"))
    )
);

//fn query_db(mut db: Database, mut q: Query) -> Result<Vec<TikaDocument>, Report> {
pub fn query_db(mut q: Query) -> Result<Vec<TikaDocument>, Report> {
    // TODO Reuse existing DB instead of creating a new one on each query
    let mut db = Database::new_with_path("mydb", DB_CREATE_OR_OVERWRITE)?;
    let mut enq = db.new_enquire()?;
    enq.set_query(&mut q)?;
    // TODO set this based on terminal height?
    let mut mset = enq.get_mset(0, 100)?;

    // TODO with verbose logging log this:
    //let appx_matches = mset.get_matches_estimated()?;
    //println!("Approximate Matches {}", appx_matches);

    let mut matches = Vec::new();
    let mut v = mset.iterator().unwrap();
    while v.is_next().unwrap() {
        let res = v.get_document_data();
        // Can use flatten() or some other iterators/combinators?
        if let Ok(data) = res {
            let v: TikaDocument = serde_json::from_str(&data)?;
            //println!("Match {}", v.filename);
            matches.push(v);
        }
        v.next()?;
    }

    Ok(matches)
}
