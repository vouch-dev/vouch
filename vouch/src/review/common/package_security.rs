use serde;

use crate::review::common::rating;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum PackageSecurity {
    VeryDangerous,
    Dangerous,
    Neutral,
    Safe,
    VerySafe,

    Unset,
}

impl PackageSecurity {
    pub fn to_rating(&self) -> rating::Rating {
        match self {
            Self::VerySafe => rating::Rating::VeryHigh,
            Self::Safe => rating::Rating::High,
            Self::Neutral => rating::Rating::Neutral,
            Self::Dangerous => rating::Rating::Low,
            Self::VeryDangerous => rating::Rating::VeryLow,

            Self::Unset => rating::Rating::Unset,
        }
    }

    pub fn to_natural_string(&self) -> String {
        match self {
            Self::VerySafe => "very safe",
            Self::Safe => "safe",
            Self::Neutral => "neutral",
            Self::Dangerous => "dangerous",
            Self::VeryDangerous => "very dangerous",

            Self::Unset => "unset",
        }
        .to_string()
    }
}

impl From<rating::Rating> for PackageSecurity {
    fn from(rating: rating::Rating) -> Self {
        match rating {
            rating::Rating::VeryHigh => Self::VerySafe,
            rating::Rating::High => Self::Safe,
            rating::Rating::Neutral => Self::Neutral,
            rating::Rating::Low => Self::Dangerous,
            rating::Rating::VeryLow => Self::VeryDangerous,

            rating::Rating::Unset => Self::Unset,
        }
    }
}

impl std::fmt::Display for PackageSecurity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{rating} ({natural_string})",
            rating = self.to_rating().to_string(),
            natural_string = self.to_natural_string()
        )
    }
}

// TODO: Is this used?
impl Default for PackageSecurity {
    fn default() -> PackageSecurity {
        PackageSecurity::Unset
    }
}

impl serde::Serialize for PackageSecurity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let rating = self.to_rating();
        serializer.serialize_str(&rating.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for PackageSecurity {
    fn deserialize<D>(deserializer: D) -> Result<PackageSecurity, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let rating = deserializer.deserialize_str(rating::Visitor)?;
        Ok(PackageSecurity::from(rating))
    }
}
