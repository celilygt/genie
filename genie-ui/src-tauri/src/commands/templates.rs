//! Template gallery commands

use super::CommandError;
use crate::state::AppState;
use genie_core::templates::{self, InputVarType, PromptTemplate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[derive(Debug, Serialize)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    pub json_output: bool,
    pub variables: Vec<VariableInfo>,
}

#[derive(Debug, Serialize)]
pub struct VariableInfo {
    pub name: String,
    pub description: String,
    pub var_type: String,
    pub default: Option<String>,
    pub required: bool,
}

impl From<&PromptTemplate> for TemplateInfo {
    fn from(t: &PromptTemplate) -> Self {
        Self {
            name: t.name.clone(),
            description: t.description.clone(),
            model: t.model.clone(),
            json_output: t.json_output,
            variables: t
                .input_variables
                .iter()
                .map(|v| VariableInfo {
                    name: v.name.clone(),
                    description: v.description.clone(),
                    var_type: match &v.var_type {
                        InputVarType::String => "string".to_string(),
                        InputVarType::File => "file".to_string(),
                        InputVarType::Number => "number".to_string(),
                        InputVarType::Boolean => "boolean".to_string(),
                        InputVarType::Enum(opts) => format!("enum:{}", opts.join(",")),
                    },
                    default: v.default.clone(),
                    required: v.required,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateDetail {
    pub info: TemplateInfo,
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct RunTemplateRequest {
    pub name: String,
    pub variables: HashMap<String, String>,
    pub files: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct RunTemplateResponse {
    pub output: String,
}

#[tauri::command]
pub async fn list_templates() -> Result<Vec<TemplateInfo>, CommandError> {
    let templates = templates::load_templates().map_err(|e| CommandError::new(e.to_string()))?;

    Ok(templates.iter().map(TemplateInfo::from).collect())
}

#[tauri::command]
pub async fn get_template(name: String) -> Result<TemplateDetail, CommandError> {
    let template = templates::find_template(&name).map_err(|e| CommandError::new(e.to_string()))?;

    Ok(TemplateDetail {
        info: TemplateInfo::from(&template),
        body: template.body.clone(),
    })
}

#[tauri::command]
pub async fn run_template(
    state: State<'_, Arc<RwLock<AppState>>>,
    request: RunTemplateRequest,
) -> Result<RunTemplateResponse, CommandError> {
    let state = state.read().await;

    let template =
        templates::find_template(&request.name).map_err(|e| CommandError::new(e.to_string()))?;

    let file_paths: HashMap<String, PathBuf> = request
        .files
        .into_iter()
        .map(|(k, v)| (k, PathBuf::from(v)))
        .collect();

    let output =
        templates::run_template(&state.gemini, &template, request.variables, file_paths, None)
            .await
            .map_err(|e| CommandError::new(e.to_string()))?;

    Ok(RunTemplateResponse { output })
}

