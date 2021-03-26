#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Position {
    pub line: i64,
    pub character: i64,
}

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Summary {
    Pass,
    Warn,
    Fail,
}

impl std::str::FromStr for Summary {
    type Err = anyhow::Error;
    fn from_str(input: &str) -> Result<Summary, Self::Err> {
        match input {
            "pass" => Ok(Summary::Pass),
            "warn" => Ok(Summary::Warn),
            "fail" => Ok(Summary::Fail),
            _ => Err(anyhow::format_err!(
                "Failed to parse comment summary type from string."
            )),
        }
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct Comment {
    #[serde(skip)]
    pub id: crate::common::index::ID,
    #[serde(rename = "file")]
    pub path: std::path::PathBuf,
    pub summary: Summary,
    #[serde(rename = "description")]
    pub message: String,
    pub selection: Option<Selection>,
}

impl crate::common::index::Identify for Comment {
    fn id(&self) -> crate::common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut crate::common::index::ID {
        &mut self.id
    }
}
