use anyhow::{format_err, Result};

use crate::common::StoreTransaction;
use crate::review;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependancyReport {
    pub summary: review::Summary,
    pub name: String,
    pub version: Option<String>,
    pub review_count: Option<u32>,
    pub note: Option<String>,
}

/// Given a local project dependancy, create a corresponding review report from known reviews.
pub fn get_dependancy_report(
    dependancy: &vouch_lib::extension::LocalDependancy,
    tx: &StoreTransaction,
) -> Result<DependancyReport> {
    if dependancy.version_parse_error || dependancy.version.is_none() {
        let note = match &dependancy.version {
            Some(v) => format!("Failed to parse version: {}", v).to_string(),
            None => "Version not found.".to_string(),
        };

        return Ok(DependancyReport {
            summary: review::Summary::Warn,
            name: dependancy.name.clone(),
            version: None,
            review_count: None,
            note: Some(note),
        });
    }
    let package_version = dependancy.version.clone().ok_or(format_err!(
        "Code error: dependancy.version_parse_error is false \
        but dependancy.version is None."
    ))?;

    let reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(&dependancy.name),
            package_version: Some(&package_version),
            registry_host_name: Some(dependancy.registry_host_name.as_str()),
            ..Default::default()
        },
        &tx,
    )?;

    if reviews.is_empty() {
        // Report no reviews found for dependancy.
        return Ok(DependancyReport {
            summary: review::Summary::Warn,
            name: dependancy.name.clone(),
            version: Some(package_version.clone()),
            review_count: Some(0),
            note: None,
        });
    }

    let stats = get_dependancy_stats(&reviews)?;
    let status = get_dependancy_status(&stats)?;
    let note = get_dependancy_note(&stats)?;

    Ok(DependancyReport {
        summary: status,
        name: dependancy.name.clone(),
        version: Some(package_version.clone()),
        review_count: Some(reviews.len() as u32),
        note: Some(note),
    })
}

#[derive(Debug, Default, Clone)]
struct DependancyStats {
    pub count_very_dangerous: u32,
    pub count_dangerous: u32,
    pub total_review_count: u32,
}

fn get_dependancy_stats(reviews: &Vec<review::Review>) -> Result<DependancyStats> {
    let mut stats = DependancyStats::default();
    stats.total_review_count = reviews.len() as u32;

    for review in reviews {
        if review.package_security == review::PackageSecurity::VeryDangerous {
            stats.count_very_dangerous += 1;
        } else if review.package_security == review::PackageSecurity::Dangerous {
            stats.count_dangerous += 1;
        }
    }
    Ok(stats)
}

fn get_dependancy_status(stats: &DependancyStats) -> Result<review::Summary> {
    if stats.count_very_dangerous > 0 || stats.count_dangerous > 0 {
        return Ok(review::Summary::Fail);
    }
    if stats.total_review_count == 0 {
        return Ok(review::Summary::Warn);
    }
    Ok(review::Summary::Pass)
}

fn get_dependancy_note(stats: &DependancyStats) -> Result<String> {
    let mut note = String::new();
    if stats.count_very_dangerous > 0 {
        note.push_str(
            format!(
                "very dangerous ({review_count})",
                review_count = stats.count_very_dangerous
            )
            .as_str(),
        );
    }

    if stats.count_dangerous > 0 {
        if !note.is_empty() {
            note.push_str("; ");
        }
        note.push_str(
            format!(
                "dangerous ({review_count})",
                review_count = stats.count_dangerous
            )
            .as_str(),
        );
    }

    Ok(note)
}
