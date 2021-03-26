use anyhow::Result;
use std::collections::HashSet;
use std::hash::Hasher;

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

/// Return values which are in primary but not in secondary. Ignores ID values.
pub fn get_difference_sans_id<T>(primary: &HashSet<T>, secondary: &HashSet<T>) -> Result<HashSet<T>>
where
    T: crate::common::HashSansId + Identify + Clone + Eq + PartialEq + std::hash::Hash,
{
    let primary = primary
        .into_iter()
        .map(|x| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            x.hash_sans_id(&mut state);
            (state.finish(), x)
        })
        .collect::<std::collections::HashMap<_, _>>();
    let secondary = secondary
        .into_iter()
        .map(|x| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            x.hash_sans_id(&mut state);
            (state.finish(), x)
        })
        .collect::<std::collections::HashMap<_, _>>();

    let primary_keys = primary.keys().collect::<HashSet<_>>();
    let secondary_keys = secondary.keys().collect::<HashSet<_>>();
    let difference_keys = primary_keys.difference(&secondary_keys);

    let difference = difference_keys
        .into_iter()
        .map(|k| primary[k].clone())
        .collect();
    Ok(difference)
}
