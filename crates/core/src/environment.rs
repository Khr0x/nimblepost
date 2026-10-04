use crate::{Document, Error, LoadedRequest, Result, collection::reject_active};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Default)]
pub struct ExecutionContext {
    pub overrides: BTreeMap<String, String>,
    pub secrets: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableOrigin {
    pub scope: &'static str,
    pub path: PathBuf,
    pub secret: bool,
}

/// Editor metadata. Secret values never leave the execution context.
#[derive(serde::Serialize)]
pub struct VariablePreview {
    pub name: String,
    pub value: Option<String>,
    pub scope: &'static str,
    pub path: PathBuf,
    pub secret: bool,
}

pub fn inherited_variables(loaded: &LoadedRequest) -> Result<Vec<VariablePreview>> {
    Ok(Variables::inherited(loaded, &ExecutionContext::default())?
        .values
        .into_iter()
        .map(|(name, variable)| VariablePreview {
            name,
            value: variable.value,
            scope: variable.origin.scope,
            path: variable.origin.path,
            secret: variable.origin.secret,
        })
        .collect())
}

struct Variable {
    value: Option<String>,
    origin: VariableOrigin,
}

pub(crate) struct Variables {
    values: BTreeMap<String, Variable>,
}

impl Variables {
    pub(crate) fn new(loaded: &LoadedRequest, context: &ExecutionContext) -> Result<Self> {
        let mut resolver = Self::inherited(loaded, context)?;
        resolver.add(
            &loaded.request,
            &loaded.request.value["runtime"]["variables"],
            "request",
            context,
        )?;
        for (name, value) in &context.overrides {
            validate_name(name).map_err(|_| {
                loaded
                    .request
                    .error("overrides", "Nombre de variable inválido")
            })?;
            let secret = resolver.values.get(name).is_some_and(|v| v.origin.secret)
                || context.secrets.contains_key(name);
            resolver.values.insert(
                name.clone(),
                Variable {
                    value: Some(value.clone()),
                    origin: VariableOrigin {
                        scope: "runtime",
                        path: loaded.request.path.clone(),
                        secret,
                    },
                },
            );
        }
        Ok(resolver)
    }

    fn inherited(loaded: &LoadedRequest, context: &ExecutionContext) -> Result<Self> {
        let mut resolver = Self {
            values: BTreeMap::new(),
        };
        for (index, doc) in loaded.defaults.iter().enumerate() {
            resolver.add(
                doc,
                &doc.value["request"]["variables"],
                if index == 0 { "collection" } else { "folder" },
                context,
            )?;
        }
        if let Some(doc) = &loaded.environment {
            reject_active(
                doc,
                &doc.value,
                &[
                    "extends",
                    "dotEnvFilePath",
                    "externalSecrets",
                    "clientCertificates",
                ],
            )?;
            resolver.add(doc, &doc.value["variables"], "environment", context)?;
        }
        Ok(resolver)
    }

    fn add(
        &mut self,
        doc: &Document,
        variables: &Value,
        scope: &'static str,
        context: &ExecutionContext,
    ) -> Result<()> {
        let mut seen = BTreeSet::new();
        for item in variables.as_array().into_iter().flatten() {
            if item["disabled"] == true {
                continue;
            }
            let name = item["name"]
                .as_str()
                .ok_or_else(|| doc.error("variables.name", "Nombre de variable obligatorio"))?;
            validate_name(name)
                .map_err(|_| doc.error("variables.name", "Nombre de variable inválido"))?;
            if !seen.insert(name) {
                return Err(doc.error(
                    "variables.name",
                    "Variable activa duplicada en el mismo scope",
                ));
            }
            let secret = item["secret"] == true;
            let value = if secret {
                if item["type"] != "string" {
                    return Err(
                        doc.error("variables.type", "Solo secretos string están soportados")
                    );
                }
                context.secrets.get(name).cloned()
            } else if let Some(value) = item["value"].as_str() {
                Some(value.to_owned())
            } else if item["value"]["type"] == "string" {
                Some(
                    item["value"]["data"]
                        .as_str()
                        .ok_or_else(|| doc.error("variables.value.data", "Se requiere string"))?
                        .to_owned(),
                )
            } else {
                return Err(doc.error(
                    "variables.value",
                    "Solo valores string sin variantes están soportados",
                ));
            };
            self.values.insert(
                name.to_owned(),
                Variable {
                    value,
                    origin: VariableOrigin {
                        scope,
                        path: doc.path.clone(),
                        secret,
                    },
                },
            );
        }
        Ok(())
    }

    pub(crate) fn interpolate(&self, input: &str, field: &str) -> Result<String> {
        let mut rest = input;
        let mut output = String::new();
        while let Some(start) = rest.find("{{") {
            let prefix = &rest[..start];
            if prefix.contains("}}") {
                return Err(Error::config("", field, "Placeholder mal formado"));
            }
            output.push_str(prefix);
            let tail = &rest[start + 2..];
            let end = tail
                .find("}}")
                .ok_or_else(|| Error::config("", field, "Placeholder sin cierre"))?;
            let name = tail[..end].trim();
            validate_name(name)
                .map_err(|_| Error::config("", field, "Nombre de placeholder inválido"))?;
            let value = self
                .values
                .get(name)
                .and_then(|v| v.value.as_ref())
                .ok_or_else(|| Error::MissingVariable {
                    name: name.to_owned(),
                    field: field.to_owned(),
                })?;
            if value.contains("{{") || value.contains("}}") {
                return Err(Error::config(
                    "",
                    field,
                    "Variables recursivas no están soportadas",
                ));
            }
            output.push_str(value);
            rest = &tail[end + 2..];
        }
        if rest.contains("}}") {
            return Err(Error::config("", field, "Placeholder mal formado"));
        }
        output.push_str(rest);
        Ok(output)
    }

    pub(crate) fn origins(&self) -> BTreeMap<String, VariableOrigin> {
        self.values
            .iter()
            .map(|(name, v)| (name.clone(), v.origin.clone()))
            .collect()
    }
}

fn validate_name(name: &str) -> std::result::Result<(), ()> {
    if !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        Ok(())
    } else {
        Err(())
    }
}
