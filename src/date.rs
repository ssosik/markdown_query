use chrono::{offset, DateTime, NaiveDateTime, Utc};
use color_eyre::Report;
use eyre::{eyre, Result};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::str::FromStr;
use std::{fmt, marker::PhantomData};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Date(i64);

#[derive(Debug)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Date {
    pub fn new(d: i64) -> Date {
        Date(d)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Create a NaiveDateTime from the timestamp
        let naive = NaiveDateTime::from_timestamp(self.0, 0);

        // Create a normal DateTime from the NaiveDateTime
        let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);

        // Format the datetime how you want
        write!(f, "{}", datetime.with_timezone(&offset::Local).to_rfc3339())
    }
}

impl FromStr for Date {
    type Err = Report;

    fn from_str(s: &str) -> Result<Date, Self::Err> {
        if let Ok(rfc3339) = DateTime::parse_from_rfc3339(s) {
            Ok(Date::new(rfc3339.timestamp()))
        } else if let Ok(s) = DateTime::parse_from_str(s, &String::from("%Y-%m-%dT%T%z")) {
            Ok(Date::new(s.timestamp()))
        } else if let Ok(s) = s.parse::<i64>() {
            Ok(Date::new(s))
        } else {
            Err(eyre!("❌ Failed to convert {} to str", s))
        }
    }
}

/// Support Deserializing a date from either a string or i64
pub fn date_deserializer<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Date;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("Epoch seconds as i64 or RFC 3339 time string")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Date::from_str(value).unwrap())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Date::new(value))
        }

        fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Date::new(value as i64))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Date::new(value as i64))
        }

        fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Date::new(value as i64))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}
