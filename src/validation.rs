use std::collections::HashMap;

use validator::ValidationErrors;

pub fn format_errors(errors: &ValidationErrors) -> HashMap<String, Vec<String>> {
    let mut formatted = HashMap::new();

    for (field, error_kind) in errors.field_errors() {
        let messages: Vec<String> = error_kind
            .iter()
            .map(|e| {
                e.message
                    .clone()
                    .map(|m| m.into_owned())
                    .unwrap_or_else(|| "Invalid field".to_string())
            })
            .collect();

        formatted.insert(field.to_string(), messages);
    }

    formatted
}
