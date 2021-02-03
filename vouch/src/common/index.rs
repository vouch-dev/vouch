use anyhow::Result;
use std::collections::HashSet;

pub type ID = i64;

/// Returns correctly formatted SQL LIKE clause match pattern.
pub fn get_like_clause_param(value: Option<&str>) -> String {
    match value {
        Some(value) => like_escape(value),
        None => "%".to_string(),
    }
}

/// Escape SQL LIKE clause.
fn like_escape(x: &str) -> String {
    x.replace("_", r"\_").replace("%", r"\%")
}

pub trait Identify {
    fn id(&self) -> ID;
    fn id_mut(&mut self) -> &mut ID;
}

pub fn get_id_neutral_set_difference<T>(
    primary: &HashSet<T>,
    secondary: &HashSet<T>,
) -> Result<HashSet<T>>
where
    T: Identify + Clone + Eq + PartialEq + std::hash::Hash,
{
    let primary: HashSet<T> = primary
        .iter()
        .cloned()
        .map(|mut x| {
            *x.id_mut() = 0;
            x
        })
        .collect();
    let secondary: HashSet<T> = secondary
        .iter()
        .cloned()
        .map(|mut x| {
            *x.id_mut() = 0;
            x
        })
        .collect();
    Ok(primary.difference(&secondary).cloned().collect())
}
