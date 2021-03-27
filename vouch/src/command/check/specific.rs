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
        println!("No reviews found.");
        let disabled_extension_names = extension::get_disabled_extension_names(&config)?;
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

        for review in reviews_by_version.get(package_version).unwrap_or(&vec![]) {
            let peer_url = if !review.peer.is_root() {
                format!("({})", review.peer.git_url)
            } else {
                "".to_string()
            };
            println!(
                "\
                \t\tPeer:              {peer_alias} {peer_url}\n\
                \t\tPackage security:  {package_security}\n\
                \t\tReview confidence: {review_confidence}\n\
            \n\n",
                peer_alias = review.peer.alias,
                peer_url = peer_url,
                package_security = review.package_security,
                review_confidence = review.review_confidence,
            );
        }
    }
    Ok(())
}

fn get_package_reviews(
    package_name: &str,
    package_version: &Option<String>,
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<std::collections::BTreeSet<review::Review>> {
    let registries = extension::get_enabled_registry_host_names(&extension_names, &config)?;

    let mut reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(package_name),
            package_version: package_version.as_deref(),
            ..Default::default()
        },
        &tx,
    )?;
    reviews.sort_by_cached_key(|review| {
        (
            review.package_security.clone(),
            review.review_confidence.clone(),
        )
    });
    let reviews = reviews
        .into_iter()
        .filter(|r| registries.contains(&r.package.registry.host_name))
        .collect::<std::collections::BTreeSet<_>>();
    Ok(reviews)
}
