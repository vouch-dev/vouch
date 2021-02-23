use std::collections::BTreeSet;
use std::io::Read;

use anyhow::{format_err, Context, Result};
use common::StoreTransaction;
use structopt::{self, StructOpt};
use tempdir;

use crate::common;
use crate::extension;
use crate::package;
use crate::peer;
use crate::review;
use crate::store;

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
    extension::update_config()?;

    let config = common::config::Config::load()?;
    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    let extension_names = handle_extension_names_arg(&args.extension_names, &config)?;

    let review = get_existing_review(
        &args.package_name,
        &args.package_version,
        &extension_names,
        &tx,
    )?;

    // Start a new review if existing review unfound.
    let review = match review {
        Some(review) => review,
        None => {
            log::debug!(
                "No existing review found. Find package remote metadata and start new review."
            );

            get_new_review(
                &args.package_name,
                &args.package_version,
                &extension_names,
                &config,
                &tx,
            )?
        }
    };

    let review = add_user_input(&review)?;

    review::index::update(&review, &tx)?;
    review::fs::add(&review, None)?;

    let commit_message = format!(
        "Add review: {registry_host_name}/{package_name}/{package_version}",
        registry_host_name = review.package.registry.host_name,
        package_name = review.package.name,
        package_version = review.package.version,
    );
    tx.commit(&commit_message)?;
    Ok(())
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

fn add_userguide_comments(review: &str) -> Result<String> {
    let mut review_and_comments = String::new();
    for line in review.lines() {
        if line.starts_with("package-security") {
            review_and_comments
                .push_str("# What are the security implications of using this package?\n");
            review_and_comments.push_str(
                "# (1) very dangerous, (2) dangerous, (3) neutral, (4) safe, (5) very safe\n",
            );
            review_and_comments.push_str("# Example: 3/5\n");
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
            review_and_comments.push_str("\n");
        } else if line.starts_with("review-confidence") {
            review_and_comments.push_str("# How confident are you in your review?\n");
            review_and_comments
                .push_str("# (1) very low, (2) low, (3) neutral, (4) high, (5) very high\n");
            review_and_comments.push_str("# Example: 3/5\n");
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
            review_and_comments.push_str("\n\n");
        } else {
            review_and_comments.push_str(format!("{line}\n", line = line).as_str());
        }
    }
    Ok(review_and_comments)
}

/// Returns the command line editor used by Git.
fn get_git_editor() -> Result<String> {
    Ok(match std::env::var("GIT_EDITOR") {
        Ok(k) => k,
        Err(_) => {
            std::env::var("EDITOR").context("Failed to obtain text editor for editing review.")?
        }
    })
}

/// Validate and clean user input.
fn clean_user_input(edited_review: &str) -> Result<review::Review> {
    let edited_review: review::Review = serde_yaml::from_str(edited_review)?;

    if edited_review.package_security == review::PackageSecurity::Unset {
        return Err(format_err!(
            "Field `package-security` not set correctly. Example expected symtax: 1/5"
        ));
    }
    if edited_review.review_confidence == review::ReviewConfidence::Unset {
        return Err(format_err!(
            "Field `review-confidence` not set correctly. Example expected symtax: 1/5"
        ));
    }
    Ok(edited_review)
}

fn add_user_input(review: &review::Review) -> Result<review::Review> {
    log::debug!("Add user input to review.");

    let temporary_directory = tempdir::TempDir::new("vouch")?;
    let file_path = temporary_directory.path().join("user_edit_review.yaml");

    {
        let mut review = serde_yaml::to_string(&review)?;
        review = add_userguide_comments(&review)?;
        std::fs::write(&file_path, &review)?;
    }

    // TODO: This doesn't work whilst debugging. Find a solution.
    let editor = get_git_editor()?;
    std::process::Command::new(&editor)
        .arg(&file_path)
        .status()
        .expect(
            format!(
                "Failed to open review for user editing. Attempted to use editor: {}",
                editor
            )
            .as_str(),
        );

    let mut file = std::fs::OpenOptions::new().read(true).open(&file_path)?;
    let mut file_contents = String::new();
    file.read_to_string(&mut file_contents)
        .context("Cannot read edited review from tmp directory.")?;
    let mut edited_review = clean_user_input(&file_contents)?;
    edited_review.id = review.id;
    edited_review.peer = review.peer.clone();
    edited_review.package = review.package.clone();
    Ok(edited_review)
}
