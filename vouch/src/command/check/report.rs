use anyhow::Result;

use crate::common::StoreTransaction;
use crate::review;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyReport {
    pub summary: review::Summary,
    pub name: String,
    pub version: Option<String>,
    pub review_count: Option<usize>,
    pub note: Option<String>,
}

/// Given a local project dependency, create a corresponding review report from known reviews.
pub fn get_dependency_report(
    dependency: &vouch_lib::extension::Dependency,
    registry_host_name: &str,
    tx: &StoreTransaction,
) -> Result<DependencyReport> {
    let package_version = match &dependency.version {
        Ok(version) => version.clone(),
        Err(error) => {
            return Ok(DependencyReport {
                summary: review::Summary::Warn,
                name: dependency.name.clone(),
                version: None,
                review_count: None,
                note: Some(error.message()),
            });
        }
    };

    let reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(&dependency.name),
            package_version: Some(&package_version),
            registry_host_names: Some(maplit::btreeset! {registry_host_name}),
            ..Default::default()
        },
        &tx,
    )?;

    if reviews.is_empty() {
        // Report no reviews found for dependency.
        return Ok(DependencyReport {
            summary: review::Summary::Todo,
            name: dependency.name.clone(),
            version: Some(package_version.clone()),
            review_count: Some(0),
            note: None,
        });
    }

    let stats = get_dependency_stats(&reviews)?;
    let status = get_dependency_status(&stats)?;
    let note = get_dependency_note(&stats)?;

    Ok(DependencyReport {
        summary: status,
        name: dependency.name.clone(),
        version: Some(package_version.clone()),
        review_count: Some(reviews.len()),
        note: Some(note),
    })
}

#[derive(Debug, Default, Clone)]
struct DependencyStats {
    pub total_review_count: usize,
    pub count_fail_comments: i32,
    pub count_warn_comments: i32,
}

fn get_dependency_stats(reviews: &Vec<review::Review>) -> Result<DependencyStats> {
    let mut stats = DependencyStats::default();
    stats.total_review_count = reviews.len();

    for review in reviews {
        let review_analysis = review::analyse(&review)?;
        stats.count_fail_comments += review_analysis.count_fail_comments;
        stats.count_warn_comments += review_analysis.count_warn_comments;
    }
    Ok(stats)
}

fn get_dependency_status(stats: &DependencyStats) -> Result<review::Summary> {
    if stats.count_fail_comments > 0 {
        return Ok(review::Summary::Fail);
    }
    if stats.total_review_count == 0 || stats.count_warn_comments > 0 {
        return Ok(review::Summary::Warn);
    }
    Ok(review::Summary::Pass)
}

fn get_dependency_note(stats: &DependencyStats) -> Result<String> {
    let mut note_parts = Vec::<_>::new();
    if stats.count_fail_comments > 0 {
        note_parts.push(format!("fail ({})", stats.count_fail_comments));
    }

    if stats.count_warn_comments > 0 {
        note_parts.push(format!("warn ({})", stats.count_warn_comments));
    }

    Ok(note_parts.join("; "))
}
