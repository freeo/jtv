use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub recipes: Vec<Recipe>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Project {
    pub fn recipe(&self, namepath: &str) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.namepath == namepath)
    }

    pub fn filtered_by_module(mut self, module: Option<&str>) -> Self {
        if let Some(module) = module {
            let prefix = format!("{}::", module.trim_end_matches("::"));
            self.recipes
                .retain(|recipe| recipe.namepath.starts_with(&prefix));
        }
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub namepath: String,
    pub doc: Option<String>,
    pub group: Option<String>,
    pub module: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub shebang: bool,
    /// Set for alias entries; execution still uses `namepath` so `just` resolves it.
    #[serde(default)]
    pub alias_target: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    /// Evaluated literal default when `just` exposes it as a string.
    pub default: Option<String>,
    /// Opaque JSON expression for non-literal defaults. It is never executed or passed as a value.
    #[serde(default)]
    pub default_expression: Option<String>,
    #[serde(default)]
    pub kind: ParameterKind,
    #[serde(default)]
    pub flag: bool,
    pub long: Option<String>,
    pub short: Option<String>,
    pub help: Option<String>,
    pub value: Option<String>,
    /// Destructuring pattern, retained as JSON when it is not a string.
    #[serde(default)]
    pub pattern: Option<String>,
}

impl Parameter {
    pub fn has_default(&self) -> bool {
        self.default.is_some() || self.default_expression.is_some()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParameterKind {
    #[default]
    Singular,
    Plus,
    Star,
    #[serde(untagged)]
    Other(String),
}
