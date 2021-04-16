use anyhow::Result;

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VersionError(String);

impl VersionError {
    pub fn from_missing_version() -> Self {
        Self("Missing version number".to_string())
    }

    pub fn from_parse_error(raw_version_number: &str) -> Self {
        Self(format!("Version parse error: {}", raw_version_number))
    }

    pub fn message(&self) -> String {
        self.0.clone()
    }
}

pub type VersionParseResult = std::result::Result<String, VersionError>;

/// A dependency as specified within a dependencies definition file.
#[derive(Clone, Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: VersionParseResult,
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
pub struct RegistryPackageMetadata {
    pub registry_host_name: String,
    pub human_url: String,
    pub artifact_url: String,

    // True if this registry is the primary registry, otherwise false.
    pub is_primary: bool,
}

pub trait FromLib: Send + Sync {
    /// Initialize extension from a library.
    fn new() -> Self
    where
        Self: Sized;
}

pub trait FromProcess: Send + Sync {
    /// Initialize extension from a process.
    fn from_process(
        process_path: &std::path::PathBuf,
        extension_config_path: &std::path::PathBuf,
    ) -> Result<Self>
    where
        Self: Sized;
}

pub trait Extension: Send + Sync {
    fn name(&self) -> String;
    fn registries(&self) -> Vec<String>;

    /// Identify local package dependencies.
    fn identify_local_dependencies(
        &self,
        working_directory: &std::path::PathBuf,
    ) -> Result<Vec<DependenciesSpec>>;

    /// Query package registries for package metadata.
    fn registries_package_metadata(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<Vec<RegistryPackageMetadata>>;
}
