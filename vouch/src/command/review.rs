use std::collections::BTreeSet;

use anyhow::{format_err, Result};
use common::StoreTransaction;
use structopt::{self, StructOpt};

use crate::common;
use crate::extension;
use crate::package;
use crate::peer;
use crate::registry;
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

    let mut config = common::config::Config::load()?;
    extension::update_config(&mut config)?;
    review::tool::check_install(&mut config)?;
    let config = config;

    let extension_names = extension::handle_extension_names_arg(&args.extension_names, &config)?;

    let mut store = store::Store::from_root()?;
    let tx = store.get_transaction()?;

    let (mut review, edit_mode, workspace_directory) = setup_review(
        &args.package_name,
        &args.package_version,
        &extension_names,
        &config,
        &tx,
    )?;

    // TODO: Make use of workspace analysis in review.
    review::workspace::analyse(&workspace_directory)?;

    let reviews_directory = review::tool::ensure_reviews_directory(&workspace_directory)?;
    let active_review_file = review::active::ensure(&review, &reviews_directory)?;

    println!("Starting review tool.");
    review::tool::run(&workspace_directory, &config)?;
    review.comments = get_comments(&active_review_file, &tx)?;
    println!(
        "Review tool closed. Fund {} review comments.",
        review.comments.len()
    );

    if review.comments.is_empty() {
        println!("No review comments found. Review saved as ongoing.");
        return Ok(());
    }

    if dialoguer::Confirm::new()
        .with_prompt("Is the review ready to share?")
        .interact()?
    {
        review::store(&review, &tx)?;
        let commit_message = get_commit_message(&review.package, &edit_mode);
        tx.commit(&commit_message)?;
        println!("Review committed.");

        review::workspace::remove(&workspace_directory)?;
    } else {
        println!("Not committing review. Review saved as ongoing.");
    }
    Ok(())
}

/// Parse user comments from active review file and insert into index.
fn get_comments(
    active_review_file: &std::path::PathBuf,
    tx: &StoreTransaction,
) -> Result<std::collections::BTreeSet<review::comment::Comment>> {
    let comments = review::active::parse(&active_review_file)?;

    let mut inserted_comments = std::collections::BTreeSet::<_>::new();
    for comment in comments {
        let comment = review::comment::index::insert(
            &comment.path,
            &comment.summary,
            &comment.message,
            &comment.selection,
            &tx,
        )?;
        inserted_comments.insert(comment);
    }

    Ok(inserted_comments)
}

/// Review edit mode.
enum ReviewEditMode {
    Create,
    Update,
}

/// Setup review for editing.
fn setup_review(
    package_name: &str,
    package_version: &str,
    extension_names: &std::collections::BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<(review::Review, ReviewEditMode, std::path::PathBuf)> {
    if let Some((review, workspace_directory)) = setup_existing_review(
        &package_name,
        &package_version,
        &extension_names,
        &config,
        &tx,
    )? {
        println!("Selecting existing review for editing.");
        Ok((review, ReviewEditMode::Update, workspace_directory))
    } else {
        println!("Starting new review.");
        let (review, workspace_directory) = setup_new_review(
            &package_name,
            &package_version,
            &extension_names,
            &config,
            &tx,
        )?;
        Ok((review, ReviewEditMode::Create, workspace_directory))
    }
}

// Setup existing review for editing.
fn setup_existing_review(
    package_name: &str,
    package_version: &str,
    extension_names: &BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<Option<(review::Review, std::path::PathBuf)>> {
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
    let reviews = filter_reviews(&reviews, &extension_names, &config)?;
    log::debug!(
        "Count existing matching reviews post filtering: {}",
        reviews.len()
    );

    if reviews.len() > 1 {
        handle_multiple_matching_reviews(&reviews, &config)?;
        return Ok(None);
    }

    let review = match reviews.first() {
        Some(review) => review,
        None => return Ok(None),
    };

    log::debug!("Setting up review workspace using existing review package metadata.");
    let (workspace_directory, _archive_hash) = review::workspace::ensure(
        &review.package.name,
        &review.package.version,
        &review.package.registry.host_name,
        &review.package.registry.artifact_url,
    )?;
    Ok(Some((review.clone(), workspace_directory)))
}

/// Filter reviews on given extension.
fn filter_reviews(
    reviews: &Vec<review::Review>,
    target_extension_names: &BTreeSet<String>,
    config: &common::config::Config,
) -> Result<Vec<review::Review>> {
    // Find registry host names which are handled by the given extensions.
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
    config: &common::config::Config,
) -> Result<()> {
    assert!(reviews.len() > 1);

    let registry_host_names: std::collections::BTreeSet<String> = reviews
        .iter()
        .map(|review| review.package.registry.host_name.clone())
        .collect();
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

/// Setup new review for editing.
fn setup_new_review(
    package_name: &str,
    package_version: &str,
    extension_names: &BTreeSet<String>,
    config: &common::config::Config,
    tx: &StoreTransaction,
) -> Result<(review::Review, std::path::PathBuf)> {
    let extensions = extension::get_enabled_extensions(&extension_names, &config)?;
    let (package, workspace_directory) =
        ensure_package_setup(&package_name, &package_version, &extensions, &tx)?;
    let review = get_insert_empty_review(&package, &tx)?;
    Ok((review, workspace_directory))
}

/// Attempt to retrieve package from index.
/// Add package metadata using extension(s) if missing.
fn ensure_package_setup(
    package_name: &str,
    package_version: &str,
    extensions: &Vec<Box<dyn vouch_lib::extension::Extension>>,
    tx: &common::StoreTransaction,
) -> Result<(package::Package, std::path::PathBuf)> {
    let remote_package_metadata = extension::search(&package_name, &package_version, &extensions)?;

    let package = package::index::get(
        &package::index::Fields {
            package_name: Some(&package_name),
            package_version: Some(&package_version),
            registry_host_name: Some(&remote_package_metadata.registry_host_name),
            ..Default::default()
        },
        &tx,
    )?
    .into_iter()
    .next();

    let package = match package {
        Some(package) => {
            let (workspace_directory, _archive_hash) = review::workspace::ensure(
                &package.name,
                &package.version,
                &package.registry.host_name,
                &package.registry.artifact_url,
            )?;
            (package, workspace_directory)
        }
        None => {
            let registry = registry::index::ensure(
                &remote_package_metadata.registry_host_name,
                &url::Url::parse(&remote_package_metadata.human_url)?,
                &url::Url::parse(&remote_package_metadata.artifact_url)?,
                &tx,
            )?;

            let (workspace_directory, archive_hash) = review::workspace::ensure(
                &package_name,
                &package_version,
                &registry.host_name,
                &registry.artifact_url,
            )?;
            let archive_hash = archive_hash.ok_or(format_err!(
                "New package object is being added to index but archive_hash is None. \
                Likely stale ongoing review."
            ))?;

            let package = package::index::insert(
                &package_name,
                &package_version,
                &registry,
                &archive_hash,
                &tx,
            )?;

            // TODO: Only cleanup archive after package insertion records archive hash.

            (package, workspace_directory)
        }
    };
    Ok(package)
}

fn get_insert_empty_review(
    package: &package::Package,
    tx: &common::StoreTransaction,
) -> Result<review::Review> {
    let root_peer =
        peer::index::get_root(&tx)?.ok_or(format_err!("Cant find root peer. Index corrupt."))?;
    let unset_review = review::index::insert(
        &std::collections::BTreeSet::<review::comment::Comment>::new(),
        &root_peer,
        &package,
        &tx,
    )?;
    Ok(unset_review)
}

fn get_commit_message(package: &package::Package, editing_mode: &ReviewEditMode) -> String {
    let message_prefix = match editing_mode {
        ReviewEditMode::Create => "Creating",
        ReviewEditMode::Update => "Updating",
    };
    format!(
        "{message_prefix} review: {registry_host_name}/{package_name}/{package_version}",
        message_prefix = message_prefix,
        registry_host_name = package.registry.host_name,
        package_name = package.name,
        package_version = package.version,
    )
}
