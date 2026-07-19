use std::collections::HashMap;

use validator::ValidationErrors;
use validator::ValidationErrorsKind;

pub fn format_errors(errors: &ValidationErrors) -> HashMap<String, Vec<String>> {
    let mut formatted = HashMap::new();

    for (field, kind) in errors.errors() {
        let messages: Vec<String> = match kind {
            ValidationErrorsKind::Field(errors) => errors
                .iter()
                .map(|e| {
                    e.message
                        .clone()
                        .map(|m| m.into_owned())
                        .unwrap_or_else(|| "Invalid field".to_string())
                })
                .collect(),
            ValidationErrorsKind::Struct(nested) => {
                // Recursively flatten nested struct errors
                format_errors(nested).into_values().flatten().collect()
            }
            ValidationErrorsKind::List(_) => continue,
        };

        if !messages.is_empty() {
            formatted.insert(field.to_string(), messages);
        }
    }

    formatted
}
