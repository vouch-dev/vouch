use anyhow::Result;

use crate::common;
use crate::common::StoreTransaction;
use crate::extension;
use crate::review;

/// Prints a report for a specific package.
pub fn report(
    package_name: &str,
    package_version: &Option<String>,
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<()> {
    // TODO: Handle multiple registries.
    let reviews = get_package_reviews(
        package_name,
        package_version,
        &extension_names,
        &config,
        &tx,
    )?;
    if reviews.is_empty() {
        if let Some(package_version) = package_version {
            println!(
                "No reviews found for package {} at version {}.",
                package_name, package_version
            );
        } else {
            println!("No reviews found for package {}.", package_name);
        }
        let disabled_extension_names = extension::manage::get_disabled_names(&config)?;
        if !disabled_extension_names.is_empty() {
            println!(
                "Consider enabling some of these extensions: {}",
                disabled_extension_names.join(", ")
            )
        }
    }

    let mut reviews_by_version = std::collections::BTreeMap::<_, Vec<&review::Review>>::new();
    for review in &reviews {
        match reviews_by_version.get_mut(&review.package.version) {
            Some(review_group) => review_group.push(&review),
            None => {
                reviews_by_version.insert(review.package.version.clone(), vec![&review]);
            }
        }
    }

    for package_version in reviews_by_version.keys().rev() {
        println!(
            "{name} {version}\n",
            name = package_name,
            version = package_version
        );

        let mut reviews = match reviews_by_version.get(package_version) {
            Some(reviews) => reviews.clone(),
            None => Vec::<_>::new(),
        };
        reviews.sort();

        let mut reports = Vec::<_>::new();
        for review in reviews {
            let report = get_review_report(&review)?;
            reports.push(report);
        }
        println!("{}", reports.join("\n\n"));
    }
    Ok(())
}

fn get_review_report(review: &review::Review) -> Result<String> {
    let peer_url = if !review.peer.is_root() {
        format!("({})", review.peer.git_url)
    } else {
        "".to_string()
    };

    Ok(format!(
        "\
Peer: {peer_alias} {peer_url}\n\
{comments_report}
\n\n",
        peer_alias = review.peer.alias,
        peer_url = peer_url,
        comments_report = serde_yaml::to_string(&review.comments)?,
    ))
}

fn get_package_reviews(
    package_name: &str,
    package_version: &Option<String>,
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<std::collections::BTreeSet<review::Review>> {
    let registry_host_names =
        extension::manage::get_enabled_registry_host_names(&extension_names, &config)?;
    let registry_host_names = registry_host_names
        .iter()
        .map(|host_name| host_name.as_str())
        .collect();

    let reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(package_name),
            package_version: package_version.as_deref(),
            registry_host_names: Some(registry_host_names),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    Ok(reviews)
}
