use serde;

use crate::review::common::rating;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum ReviewConfidence {
    VeryHigh,
    High,
    Neutral,
    Low,
    VeryLow,

    Unset,
}

impl ReviewConfidence {
    pub fn to_rating(&self) -> rating::Rating {
        match self {
            Self::VeryHigh => rating::Rating::VeryHigh,
            Self::High => rating::Rating::High,
            Self::Neutral => rating::Rating::Neutral,
            Self::Low => rating::Rating::Low,
            Self::VeryLow => rating::Rating::VeryLow,

            Self::Unset => rating::Rating::Unset,
        }
    }

    pub fn to_natural_string(&self) -> String {
        match self {
            Self::VeryHigh => "very high",
            Self::High => "high",
            Self::Neutral => "neutral",
            Self::Low => "low",
            Self::VeryLow => "very low",

            Self::Unset => "unset",
        }
        .to_string()
    }
}

impl From<rating::Rating> for ReviewConfidence {
    fn from(rating: rating::Rating) -> Self {
        match rating {
            rating::Rating::VeryHigh => Self::VeryHigh,
            rating::Rating::High => Self::High,
            rating::Rating::Neutral => Self::Neutral,
            rating::Rating::Low => Self::Low,
            rating::Rating::VeryLow => Self::VeryLow,

            rating::Rating::Unset => Self::Unset,
        }
    }
}

impl std::fmt::Display for ReviewConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{rating} ({natural_string})",
            rating = self.to_rating().to_string(),
            natural_string = self.to_natural_string()
        )
    }
}

impl Default for ReviewConfidence {
    fn default() -> ReviewConfidence {
        ReviewConfidence::Unset
    }
}

impl serde::Serialize for ReviewConfidence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let rating = self.to_rating();
        serializer.serialize_str(&rating.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for ReviewConfidence {
    fn deserialize<D>(deserializer: D) -> Result<ReviewConfidence, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let rating = deserializer.deserialize_str(rating::Visitor)?;
        Ok(ReviewConfidence::from(rating))
    }
}
