//! SkillPort + ToolRegistry — named invokable capabilities with typed schemas.
//!
//! Core defines descriptors, schema validation, and port contracts;
//! `substrate-skills` provides an in-memory registry implementation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, SubstrateError};

/// Metadata for a registered skill (tool).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDescriptor {
    /// Unique skill name used at invoke time.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema describing valid invoke input.
    pub input_schema: Value,
    /// JSON Schema describing the invoke output shape.
    pub output_schema: Value,
}

/// Handler invoked after input schema validation succeeds.
pub trait SkillHandler: Send + Sync {
    /// Run the skill with validated `input` JSON.
    fn invoke(&self, input: Value) -> Result<Value>;
}

/// Invoke registered skills and list available capabilities.
pub trait SkillPort: Send + Sync {
    /// Invoke `name` with `input`. Implementations MUST validate `input`
    /// against the skill's `input_schema` before calling the handler.
    fn invoke(&self, name: &str, input: Value) -> Result<Value>;

    /// Return descriptors for all registered skills.
    fn list_skills(&self) -> Vec<SkillDescriptor>;
}

/// Registry of named skills: register, lookup, list, and schema validation.
pub trait ToolRegistry: Send + Sync {
    /// Register a skill. Returns an error if `name` is already taken or schemas
    /// are invalid.
    fn register(
        &mut self,
        descriptor: SkillDescriptor,
        handler: Box<dyn SkillHandler>,
    ) -> Result<()>;

    /// Look up a skill descriptor by name.
    fn lookup(&self, name: &str) -> Option<&SkillDescriptor>;

    /// List all registered skill descriptors.
    fn list(&self) -> Vec<SkillDescriptor>;

    /// Validate `input` against the named skill's `input_schema`.
    fn validate_input(&self, name: &str, input: &Value) -> Result<()>;
}

/// Validate `value` against a JSON Schema subset (`type`, `properties`, `required`).
pub fn validate_json_schema(value: &Value, schema: &Value) -> Result<()> {
    let Some(schema_obj) = schema.as_object() else {
        return Err(SubstrateError::SchemaValidation(
            "schema must be a JSON object".into(),
        ));
    };

    if let Some(expected_type) = schema_obj.get("type").and_then(|t| t.as_str()) {
        let matches = match expected_type {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            other => {
                return Err(SubstrateError::SchemaValidation(format!(
                    "unsupported schema type: {other}"
                )));
            },
        };
        if !matches {
            return Err(SubstrateError::SchemaValidation(format!(
                "expected type {expected_type}, got {value}"
            )));
        }
    }

    if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
        let obj = value
            .as_object()
            .ok_or_else(|| SubstrateError::SchemaValidation("value must be an object".into()))?;
        for key in required {
            let key_str = key.as_str().ok_or_else(|| {
                SubstrateError::SchemaValidation("required entry must be a string".into())
            })?;
            if !obj.contains_key(key_str) {
                return Err(SubstrateError::SchemaValidation(format!(
                    "missing required field: {key_str}"
                )));
            }
        }
    }

    if let (Some(props), Some(obj)) = (
        schema_obj.get("properties").and_then(|p| p.as_object()),
        value.as_object(),
    ) {
        for (key, prop_schema) in props {
            if let Some(field_value) = obj.get(key) {
                validate_json_schema(field_value, prop_schema)?;
            }
        }
    }

    Ok(())
}
