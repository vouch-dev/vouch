use std::hash::Hash;

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
    Fail,
    Warn,
    Pass,
}

impl std::str::FromStr for Summary {
    type Err = anyhow::Error;
    fn from_str(input: &str) -> Result<Summary, Self::Err> {
        match input {
            "pass" => Ok(Summary::Pass),
            "warn" => Ok(Summary::Warn),
            "fail" => Ok(Summary::Fail),
            _ => Err(anyhow::format_err!(
                "Failed to parse summary type from string: {}",
                input
            )),
        }
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    #[serde(skip)]
    pub id: crate::common::index::ID,
    pub summary: Summary,
    #[serde(rename = "file")]
    pub path: std::path::PathBuf,
    #[serde(rename = "description")]
    pub message: String,
    pub selection: Option<Selection>,
}

impl Ord for Comment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.summary,
            &self.path,
            &self.message,
            &self.selection,
            &self.id,
        )
            .cmp(&(
                &other.summary,
                &other.path,
                &other.message,
                &other.selection,
                &other.id,
            ))
    }
}

impl PartialOrd for Comment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl crate::common::HashSansId for Comment {
    fn hash_sans_id<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.summary.hash(state);
        self.message.hash(state);
        self.selection.hash(state);
    }
}

impl crate::common::index::Identify for Comment {
    fn id(&self) -> crate::common::index::ID {
        self.id
    }

    fn id_mut(&mut self) -> &mut crate::common::index::ID {
        &mut self.id
    }
}
