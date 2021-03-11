#[derive(serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub line: u64,
    pub character: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum Summary {
    Pass,
    Warn,
    Fail,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub file: std::path::PathBuf,
    pub summary: Summary,
    pub description: String,
    pub selection: Selection,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetailedReview {
    pub title: String,
    pub description: String,
    pub is_primary: Option<bool>,
    pub comments: Vec<Comment>,
}
