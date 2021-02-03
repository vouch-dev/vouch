use anyhow;
use std::convert::TryFrom;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Rating {
    VeryHigh,
    High,
    Neutral,
    Low,
    VeryLow,

    Unset,
}

impl Rating {
    pub fn to_string(&self) -> String {
        match self {
            Self::VeryHigh => "5/5",
            Self::High => "4/5",
            Self::Neutral => "3/5",
            Self::Low => "2/5",
            Self::VeryLow => "1/5",

            Self::Unset => "/5",
        }
        .to_string()
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            Self::VeryHigh => 5,
            Self::High => 4,
            Self::Neutral => 3,
            Self::Low => 2,
            Self::VeryLow => 1,

            Self::Unset => 0,
        }
    }
}

impl std::convert::TryFrom<&str> for Rating {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        Ok(match value {
            "5/5" => Self::VeryHigh,
            "4/5" => Self::High,
            "3/5" => Self::Neutral,
            "2/5" => Self::Low,
            "1/5" => Self::VeryLow,
            "/5" => Self::Unset,
            _ => return Err(anyhow::format_err!("Failed to parse rating.")),
        })
    }
}

impl std::convert::TryFrom<&u8> for Rating {
    type Error = anyhow::Error;

    fn try_from(value: &u8) -> anyhow::Result<Self> {
        Ok(match value {
            5 => Self::VeryHigh,
            4 => Self::High,
            3 => Self::Neutral,
            2 => Self::Low,
            1 => Self::VeryLow,
            0 => Self::Unset,
            _ => return Err(anyhow::format_err!("Failed to parse rating.")),
        })
    }
}

impl serde::Serialize for Rating {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let rating = match self {
            Self::VeryHigh => "5/5",
            Self::High => "4/5",
            Self::Neutral => "3/5",
            Self::Low => "2/5",
            Self::VeryLow => "1/5",

            Self::Unset => "/5",
        };
        serializer.serialize_str(rating)
    }
}

pub struct Visitor;

impl<'de> serde::de::Visitor<'de> for Visitor {
    type Value = Rating;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a value out of 5, such as 1/5")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // Cleanup raw rating string.
        let valid_charecters_regex = regex::Regex::new(r"[^/1-5]").unwrap();
        let cleaned_value = valid_charecters_regex.replace_all(value, "").to_string();

        Ok(Rating::try_from(cleaned_value.as_str())
            .map_err(|_| E::custom(format!("failed to parse rating \"{}\"", value)))?)
    }
}
