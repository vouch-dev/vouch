use anyhow::{format_err, Context, Result};

mod common;
mod core;
mod extensions;
mod review_tool;

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct Config {
    pub core: core::Core,

    #[serde(rename = "review-tool")]
    pub review_tool: review_tool::ReviewTool,

    pub extensions: extensions::Extensions,
}

impl Config {
    pub fn load() -> Result<Self> {
        log::debug!("Loading config.");
        let paths = super::fs::ConfigPaths::new()?;
        log::debug!("Config paths: {:?}", paths);

        let file = std::fs::File::open(paths.config_file)?;
        let reader = std::io::BufReader::new(file);
        Ok(serde_yaml::from_reader(reader)?)
    }

    pub fn dump(&self) -> Result<()> {
        let paths = super::fs::ConfigPaths::new()?;
        if paths.config_file.is_file() {
            std::fs::remove_file(&paths.config_file)?;
        }

        let file = std::fs::OpenOptions::new()
            .write(true)
            .append(false)
            .create(true)
            .open(&paths.config_file)
            .context(format!(
                "Can't open/create file for writing: {}",
                paths.config_file.display()
            ))?;
        let writer = std::io::BufWriter::new(file);
        serde_yaml::to_writer(writer, &self)?;
        Ok(())
    }

    pub fn set(&mut self, name: &str, value: &str) -> Result<()> {
        let name_error_message = format!("Unknown settings field: {}", name);

        return if core::is_match(name)? {
            Ok(core::set(&mut self.core, &name, &value)?)
        } else if extensions::is_match(name)? {
            Ok(extensions::set(&mut self.extensions, &name, &value)?)
        } else if review_tool::is_match(name)? {
            Ok(review_tool::set(&mut self.review_tool, &name, &value)?)
        } else {
            Err(format_err!(name_error_message.clone()))
        };
    }

    pub fn get(&self, name: &str) -> Result<String> {
        let name_error_message = format!("Unknown settings field: {}", name);

        return if core::is_match(name)? {
            Ok(core::get(&self.core, &name)?)
        } else if extensions::is_match(name)? {
            Ok(extensions::get(&self.extensions, &name)?)
        } else if review_tool::is_match(name)? {
            Ok(review_tool::get(&self.review_tool, &name)?)
        } else {
            Err(format_err!(name_error_message.clone()))
        };
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_yaml::to_string(&self).map_err(|_| std::fmt::Error::default())?
        )
    }
}
