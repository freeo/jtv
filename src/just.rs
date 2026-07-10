//! Adapter for the machine-readable `just --dump --dump-format json` contract.

use std::{collections::BTreeMap, io, process::Command};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    Error, Result,
    invocation::Invocation,
    model::{Parameter, ParameterKind, Project, Recipe},
};

/// Invoke `just` exactly once and normalize its JSON dump.
pub fn load_project(invocation: &Invocation) -> Result<Project> {
    let mut command = Command::new(&invocation.just_binary);
    command
        .current_dir(&invocation.cwd)
        .args(["--dump", "--dump-format", "json"]);
    if let Some(justfile) = &invocation.justfile {
        command.arg("--justfile").arg(justfile);
    }

    let output = command.output().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            Error::MissingProgram { program: "just" }
        } else {
            Error::Spawn {
                program: invocation.just_binary.display().to_string(),
                source,
            }
        }
    })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: invocation.just_binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_project(&output.stdout, invocation.module_filter.as_deref())
}

/// Render `just --show` output for presentation only. No semantics are parsed from it.
pub fn render_preview(invocation: &Invocation, recipe: &Recipe) -> Result<String> {
    let mut command = Command::new(&invocation.just_binary);
    command.current_dir(&invocation.cwd).arg("--show");
    if let Some(justfile) = &invocation.justfile {
        command.arg("--justfile").arg(justfile);
    }
    command.arg(&recipe.namepath);
    let output = command.output().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            Error::MissingProgram { program: "just" }
        } else {
            Error::Spawn {
                program: invocation.just_binary.display().to_string(),
                source,
            }
        }
    })?;
    if !output.status.success() {
        return Err(Error::ProgramFailed {
            program: invocation.just_binary.display().to_string(),
            status: output.status.code().unwrap_or(1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse a dump captured from `just`. Public for fixture/contract testing.
pub fn parse_project(json: &[u8], module_filter: Option<&str>) -> Result<Project> {
    let dump: Dump = serde_json::from_slice(json)?;
    let mut project = Project {
        recipes: Vec::new(),
        warnings: dump.warnings.clone(),
    };
    flatten_dump(&dump, None, &mut project.recipes);
    resolve_alias_parameters(&mut project.recipes);
    project.recipes.sort_by(|a, b| a.namepath.cmp(&b.namepath));
    Ok(project.filtered_by_module(module_filter))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Dump {
    recipes: BTreeMap<String, RawRecipe>,
    aliases: BTreeMap<String, RawAlias>,
    modules: BTreeMap<String, Dump>,
    module_path: String,
    warnings: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawAlias {
    name: String,
    target: String,
    attributes: Vec<Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawRecipe {
    name: String,
    namepath: String,
    doc: Option<String>,
    dependencies: Vec<Value>,
    parameters: Vec<RawParameter>,
    body: Vec<Value>,
    attributes: Vec<Value>,
    private: bool,
    quiet: bool,
    shebang: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawParameter {
    name: String,
    default: Option<Value>,
    kind: String,
    flag: bool,
    long: Option<String>,
    short: Option<String>,
    help: Option<String>,
    value: Option<Value>,
    pattern: Option<Value>,
}

fn flatten_dump(dump: &Dump, parent: Option<&str>, output: &mut Vec<Recipe>) {
    let module = module_name(dump, parent);
    for recipe in dump.recipes.values().filter(|recipe| !recipe.private) {
        output.push(normalize_recipe(recipe, module.as_deref()));
    }
    for (key, alias) in &dump.aliases {
        let name = if alias.name.is_empty() {
            key
        } else {
            &alias.name
        };
        let namepath = qualify(module.as_deref(), name);
        output.push(Recipe {
            name: name.to_owned(),
            namepath,
            module: module.clone(),
            attributes: alias.attributes.iter().map(value_text).collect(),
            alias_target: Some(qualify(module.as_deref(), &alias.target)),
            ..Recipe::default()
        });
    }
    for (name, child) in &dump.modules {
        let fallback = qualify(module.as_deref(), name);
        flatten_dump(child, Some(&fallback), output);
    }
}

fn module_name(dump: &Dump, fallback: Option<&str>) -> Option<String> {
    if dump.module_path.is_empty() {
        fallback.map(str::to_owned)
    } else {
        Some(dump.module_path.clone())
    }
}

fn normalize_recipe(raw: &RawRecipe, module: Option<&str>) -> Recipe {
    let namepath = if raw.namepath.is_empty() {
        qualify(module, &raw.name)
    } else {
        raw.namepath.clone()
    };
    let group = raw
        .attributes
        .iter()
        .find_map(|attribute| attribute.get("group")?.as_str().map(str::to_owned));
    Recipe {
        name: raw.name.clone(),
        namepath,
        doc: raw.doc.clone(),
        group,
        module: module.map(str::to_owned),
        dependencies: raw.dependencies.iter().map(dependency_text).collect(),
        parameters: raw.parameters.iter().map(normalize_parameter).collect(),
        body: raw.body.iter().map(value_text).collect(),
        attributes: raw.attributes.iter().map(value_text).collect(),
        private: raw.private,
        quiet: raw.quiet,
        shebang: raw.shebang,
        alias_target: None,
    }
}

fn normalize_parameter(raw: &RawParameter) -> Parameter {
    let (default, default_expression) = match raw.default.as_ref() {
        Some(Value::String(value)) => (Some(value.clone()), None),
        Some(value) => (None, Some(value_text(value))),
        None => (None, None),
    };
    Parameter {
        name: raw.name.clone(),
        default,
        default_expression,
        kind: match raw.kind.as_str() {
            "" | "singular" => ParameterKind::Singular,
            "plus" => ParameterKind::Plus,
            "star" => ParameterKind::Star,
            other => ParameterKind::Other(other.to_owned()),
        },
        flag: raw.flag,
        long: raw.long.clone(),
        short: raw.short.clone(),
        help: raw.help.clone(),
        value: raw.value.as_ref().map(value_text),
        pattern: raw.pattern.as_ref().map(value_text),
    }
}

fn resolve_alias_parameters(recipes: &mut [Recipe]) {
    for _ in 0..recipes.len() {
        let targets: BTreeMap<_, _> = recipes
            .iter()
            .map(|recipe| (recipe.namepath.clone(), recipe.clone()))
            .collect();
        let mut changed = false;
        for alias in recipes
            .iter_mut()
            .filter(|recipe| recipe.alias_target.is_some())
        {
            let Some(target) = alias
                .alias_target
                .as_deref()
                .and_then(|namepath| targets.get(namepath))
            else {
                continue;
            };
            if alias.parameters != target.parameters {
                alias.parameters = target.parameters.clone();
                changed = true;
            }
            if alias.doc.is_none() {
                alias.doc = target.doc.clone();
            }
        }
        if !changed {
            break;
        }
    }
}

fn dependency_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("recipe")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            value
                .get("namepath")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| value_text(value))
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).expect("JSON value serializes"),
    }
}

fn qualify(module: Option<&str>, name: &str) -> String {
    module
        .filter(|value| !value.is_empty())
        .map_or_else(|| name.to_owned(), |module| format!("{module}::{name}"))
}
