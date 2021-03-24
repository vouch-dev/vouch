use std::collections::BTreeSet;

use anyhow::{format_err, Result};
use common::StoreTransaction;
use structopt::{self, StructOpt};

use crate::common;
use crate::extension;
use crate::package;
use crate::peer;
use crate::review;
use crate::store;

mod summary;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "no_version",
    no_version,
    global_settings = &[structopt::clap::AppSettings::DisableVersion]
)]
pub struct Arguments {
    /// Package name.
    #[structopt(name = "package-name")]
    pub package_name: String,

    /// Package version.
    #[structopt(name = "package-version")]
    pub package_version: String,

    /// Specify an extension for handling the package.
    /// Example values: py, js, rs
    #[structopt(long = "extension", short = "e", name = "name")]
    pub extension_names: Option<Vec<String>>,
}

pub fn run_command(args: &Arguments) -> Result<()> {
    // TODO: Add gpg signing.

    let mut config = common::config::Config::load()?;
    extension::update_config(&mut config)?;
    review::tool::check_install(&mut config)?;
    let config = config;

    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    let extension_names = handle_extension_names_arg(&args.extension_names, &config)?;
    let (review, editing_mode) = get_review(
        &args.package_name,
        &args.package_version,
        &extension_names,
        &config,
        &tx,
    )?;

    let workspace_directory = review::workspace::ensure(&review.package)?;
    review::workspace::analyse(&workspace_directory)?;

    let reviews_directory = review::tool::ensure_reviews_directory(&workspace_directory)?;
    let active_review_file = review::active::ensure(&review.package, &reviews_directory)?;

    review::tool::run(&workspace_directory, &config)?;

    let _comments = review::active::parse(&active_review_file)?;

    let review = summary::add_user_input(&review)?;
    review::store(&review, &tx)?;

    let commit_message = get_commit_message(&review.package, &editing_mode);
    tx.commit(&commit_message)?;
    Ok(())
}

/// Review edit mode.
enum EditingMode {
    Create,
    Update,
}

/// Retrieve existing or new review.
fn get_review(
    package_name: &str,
    package_version: &str,
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<(review::Review, EditingMode)> {
    if let Some(review) =
        get_existing_review(&package_name, &package_version, &extension_names, &tx)?
    {
        log::debug!("Existing review found.");
        Ok((review, EditingMode::Update))
    } else {
        log::debug!("No existing review found. Starting new review.");
        let review = get_new_review(
            &package_name,
            &package_version,
            &extension_names,
            &config,
            &tx,
        )?;
        Ok((review, EditingMode::Create))
    }
}

/// Check given extensions are enabled. If not specified select all enabled extensions.
fn handle_extension_names_arg(
    extension_names: &Option<Vec<String>>,
    config: &common::config::Config,
) -> Result<BTreeSet<String>> {
    let names = match &extension_names {
        Some(extension_names) => {
            let disabled_names: Vec<_> = extension_names
                .iter()
                .cloned()
                .filter(|name| !extension::is_enabled(&name, &config).unwrap_or(false))
                .collect();
            if !disabled_names.is_empty() {
                return Err(format_err!(
                    "The following disabled extensions were given: {}",
                    disabled_names.join(", ")
                ));
            } else {
                extension_names.into_iter().cloned().collect()
            }
        }
        None => extension::get_enabled_names(&config)?,
    };
    log::debug!("Using extensions: {:?}", names);
    Ok(names)
}

// Check index for existing root peer review.
fn get_existing_review(
    package_name: &str,
    package_version: &str,
    extension_names: &BTreeSet<String>,
    tx: &StoreTransaction,
) -> Result<Option<review::Review>> {
    log::debug!("Checking index for existing root peer review.");
    let root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;
    let reviews = review::index::get(
        &review::index::Fields {
            package_name: Some(&package_name),
            package_version: Some(&package_version),
            peer: Some(&root_peer),
            ..Default::default()
        },
        &tx,
    )?;

    log::debug!("Count existing matching reviews: {}", reviews.len());
    let reviews = filter_reviews(&reviews, &extension_names)?;
    log::debug!(
        "Count existing matching reviews post filtering: {}",
        reviews.len()
    );

    if reviews.len() > 1 {
        handle_multiple_matching_reviews(&reviews)
    } else {
        Ok(reviews.first().cloned())
    }
}

/// Filter reviews on given extension.
fn filter_reviews(
    reviews: &Vec<review::Review>,
    target_extension_names: &BTreeSet<String>,
) -> Result<Vec<review::Review>> {
    // Find registry host names which are handled by the given extensions.
    let config = crate::common::config::Config::load()?;
    let extension_supported_registry_host_names: std::collections::BTreeSet<String> = config
        .extensions
        .supported_package_registries
        .iter()
        .filter(|(_registry_host_name, extension_name)| {
            target_extension_names.contains(extension_name.as_str())
        })
        .map(|(registry_host_name, _extension_name)| registry_host_name.clone())
        .collect();

    Ok(reviews
        .iter()
        .filter(|review| {
            extension_supported_registry_host_names.contains(&review.package.registry.host_name)
        })
        .map(|review| review.clone())
        .collect())
}

/// Request extension specification when multiple matching reviews found.
fn handle_multiple_matching_reviews(
    reviews: &Vec<review::Review>,
) -> Result<Option<review::Review>> {
    assert!(reviews.len() > 1);

    let registry_host_names: std::collections::BTreeSet<String> = reviews
        .iter()
        .map(|review| review.package.registry.host_name.clone())
        .collect();
    let config = crate::common::config::Config::load()?;
    let extension_names: std::collections::BTreeSet<String> = config
        .extensions
        .supported_package_registries
        .iter()
        .filter(|(registry_host_name, _extension_name)| {
            registry_host_names.contains(registry_host_name.as_str())
        })
        .map(|(_registry_host_name, extension_name)| extension_name.clone())
        .collect();

    let extension_names: Vec<String> = extension_names.into_iter().collect();

    return Err(format_err!(
        "Found multiple matching candidate packages.\n\
        Please specify an extension using --extension (-e).\n\
        Matching extensions: {}",
        extension_names.join(", ")
    ));
}

/// Get new review.
fn get_new_review(
    package_name: &str,
    package_version: &str,
    extension_names: &BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<review::Review> {
    let extensions = extension::get_enabled_extensions(&config)?
        .into_iter()
        .filter(|extension| extension_names.contains(&extension.name()))
        .collect();
    let package = get_insert_package(&package_name, &package_version, &extensions, &tx)?.ok_or(
        format_err!("Failed to derive package metadata from extension(s)."),
    )?;
    get_insert_unset_review(&package, &tx)
}

/// Attempt to retrieve package from index.
/// Add package metadata using extension(s) if missing.
fn get_insert_package(
    package_name: &str,
    package_version: &str,
    extensions: &Vec<Box<dyn vouch_lib::extension::Extension>>,
    tx: &common::StoreTransaction,
) -> Result<Option<package::Package>> {
    let (_extension, remote_package_metadata) =
        extension::get_remote_package_metadata(&package_name, &package_version, &extensions)?
            .ok_or(format_err!("Failed to find package in package registries."))?;
    let registry_human_url = match &remote_package_metadata.registry_human_url {
        Some(url) => url::Url::parse(url.as_str())?,
        None => return Ok(None),
    };
    let source_code_url = url::Url::parse(
        remote_package_metadata
            .source_code_url
            .clone()
            .ok_or(format_err!("Could not find source code URL."))?
            .as_str(),
    )?;

    let source_code_hash = &remote_package_metadata
        .source_code_hash
        .clone()
        .ok_or(format_err!("Could not find source code SHA256 hash."))?;

    let registry_host_name = &remote_package_metadata
        .registry_host_name
        .clone()
        .ok_or(format_err!("Registry host name not specified."))?;

    let package = match package::index::get(
        &package::index::Fields {
            package_name: Some(&package_name),
            package_version: Some(&package_version),
            registry_host_name: Some(registry_host_name),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next()
    {
        Some(package) => package,
        None => package::index::insert(
            &package_name,
            &package_version,
            &registry_human_url,
            &source_code_url,
            &source_code_hash,
            &registry_host_name,
            &tx,
        )?,
    };
    Ok(Some(package))
}

fn get_insert_unset_review(
    package: &package::Package,
    tx: &common::StoreTransaction,
) -> Result<review::Review> {
    let root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;
    let unset_review = review::index::insert(
        &review::PackageSecurity::Unset,
        &review::ReviewConfidence::Unset,
        &root_peer,
        &package,
        &tx,
    )?;
    Ok(unset_review)
}

fn get_commit_message(package: &package::Package, editing_mode: &EditingMode) -> String {
    let message_prefix = match editing_mode {
        EditingMode::Create => "Creating",
        EditingMode::Update => "Updating",
    };
    format!(
        "{message_prefix} review: {registry_host_name}/{package_name}/{package_version}",
        message_prefix = message_prefix,
        registry_host_name = package.registry.host_name,
        package_name = package.name,
        package_version = package.version,
    )
}
