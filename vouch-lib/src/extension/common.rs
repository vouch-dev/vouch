use anyhow::Result;

/// A dependency as specified within a dependencies definition file.
#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dependency {
    pub name: String,

    // TODO: Change to result with error types.
    pub version: Option<String>,
    pub version_parse_error: bool,
    pub missing_version: bool,
}

/// A dependencies specification file found from inspecting the local filesystem.
#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DependenciesSpec {
    /// Absolute file path for dependencies specification file.
    pub path: std::path::PathBuf,

    /// Dependencies registry host name.
    pub registry_host_name: String,

    /// Dependencies specified within the dependencies specification file.
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemotePackageMetadata {
    pub registry_host_name: String,
    pub registry_human_url: String,
    pub archive_url: String,
}

pub trait Extension: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Initialize extension from a process.
    fn from_process(
        process_path: &std::path::PathBuf,
        extension_config_path: &std::path::PathBuf,
    ) -> Result<Self>
    where
        Self: Sized;

    fn name(&self) -> String;
    fn registries(&self) -> Vec<String>;

    /// Identify local package dependencies.
    fn identify_local_dependencies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<DependenciesSpec>>;

    /// Query package registries for package metadata.
    fn remote_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<RemotePackageMetadata>;
}
