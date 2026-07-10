//! Optional `.jtv.toml` metadata.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, model::Project};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub recipes: BTreeMap<String, RecipeConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecipeConfig {
    #[serde(default)]
    pub params: BTreeMap<String, ParameterConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ParameterConfig {
    String,
    Secret,
    Choice { values: Vec<String> },
    Boolean,
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: Option<PathBuf>,
    pub config: Config,
}

impl Config {
    pub fn load_upward(start: &Path) -> Result<LoadedConfig> {
        let mut directory = if start.is_dir() {
            start.to_path_buf()
        } else {
            start.parent().unwrap_or(start).to_path_buf()
        };
        loop {
            let candidate = directory.join(".jtv.toml");
            if candidate.is_file() {
                let text = std::fs::read_to_string(&candidate).map_err(|source| Error::Read {
                    path: candidate.clone(),
                    source,
                })?;
                let config = toml::from_str(&text).map_err(|error| Error::Config {
                    path: candidate.clone(),
                    message: error.to_string(),
                })?;
                return Ok(LoadedConfig {
                    path: Some(candidate),
                    config,
                });
            }
            if !directory.pop() {
                break;
            }
        }
        Ok(LoadedConfig {
            path: None,
            config: Self::default(),
        })
    }

    pub fn validate(&self, project: &Project, path: &Path) -> Result<()> {
        for (recipe_name, recipe_config) in &self.recipes {
            let recipe = project.recipe(recipe_name).ok_or_else(|| Error::Config {
                path: path.to_path_buf(),
                message: format!("unknown recipe `{recipe_name}`"),
            })?;
            for parameter in recipe_config.params.keys() {
                if !recipe
                    .parameters
                    .iter()
                    .any(|candidate| candidate.name == *parameter)
                {
                    return Err(Error::Config {
                        path: path.to_path_buf(),
                        message: format!(
                            "unknown parameter `{parameter}` for recipe `{recipe_name}`"
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn parameter(&self, recipe: &str, parameter: &str) -> Option<&ParameterConfig> {
        self.recipes.get(recipe)?.params.get(parameter)
    }
}
