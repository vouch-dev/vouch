use anyhow::Result;

/// Dependancies found from inspecting the local filesystem.
#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalDependancy {
    pub registry_host_name: String,
    pub name: String,

    // TODO: Change to result with error types.
    pub version: Option<String>,
    pub version_parse_error: bool,
    pub missing_version: bool,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemotePackageMetadata {
    pub found_local_use: bool,
    pub registry_host_name: Option<String>,
    pub registry_package_url: Option<String>,
    pub registry_human_url: Option<String>,
    pub source_code_url: Option<String>,
    pub source_code_hash: Option<String>,
}

pub trait Extension: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Initialise extension from a process.
    fn from_process(
        process_path: &std::path::PathBuf,
        extension_config_path: &std::path::PathBuf,
    ) -> Result<Self>
    where
        Self: Sized;

    fn name(&self) -> String;
    fn registries(&self) -> Vec<String>;

    /// Returns a list of local package dependancies which are
    /// relevant to this extension.
    fn identify_local_dependancies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<LocalDependancy>>;

    /// Given a package name and version, queries the remote
    /// registry for package metadata.
    fn remote_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
        working_directory: &std::path::PathBuf,
    ) -> Result<RemotePackageMetadata>;
}
