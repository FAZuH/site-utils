use std::collections::HashMap;

use dioxus::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use validator::Validate;
use validator::ValidationError;
use validator::ValidationErrors;

#[derive(Debug, Validate, Serialize, Deserialize)]
#[validate(schema(function = "validate_contact_fields"))]
pub struct ContactForm {
    #[validate(length(
        min = 2,
        max = 100,
        message = "Name must be between 2 and 100 characters"
    ))]
    pub name: String,

    #[validate(email(message = "Please enter a valid email address"))]
    pub email: Option<String>,

    pub phone: Option<String>,

    #[validate(length(
        min = 2,
        max = 200,
        message = "Subject must be between 2 and 200 characters"
    ))]
    pub subject: Option<String>,

    #[validate(length(
        min = 10,
        max = 1000,
        message = "Message must be between 10 and 1000 characters"
    ))]
    pub message: String,
}

fn validate_contact_fields(form: &ContactForm) -> Result<(), ValidationError> {
    if form.email.is_none() && form.phone.is_none() {
        let mut err = ValidationError::new("contact_required");
        err.message = Some("Please provide at least one of email or phone number".into());
        return Err(err);
    }
    Ok(())
}

// ── Builder ────────────────────────────────────────────────────────────────

impl ContactForm {
    /// Start building a ContactForm with the required name.
    pub fn builder(name: impl Into<String>) -> ContactFormBuilder {
        ContactFormBuilder::new(name)
    }
}

pub struct ContactFormBuilder {
    name: String,
    email: Option<String>,
    phone: Option<String>,
    subject: Option<String>,
    explicit_message: Option<String>,
    extras: Vec<(String, String)>,
}

impl ContactFormBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            phone: None,
            subject: None,
            explicit_message: None,
            extras: Vec::new(),
        }
    }

    pub fn email(mut self, v: impl Into<String>) -> Self {
        self.email = Some(v.into());
        self
    }

    pub fn email_opt(mut self, v: Option<String>) -> Self {
        self.email = v;
        self
    }

    pub fn phone(mut self, v: impl Into<String>) -> Self {
        self.phone = Some(v.into());
        self
    }

    pub fn phone_opt(mut self, v: Option<String>) -> Self {
        self.phone = v;
        self
    }

    pub fn subject(mut self, v: impl Into<String>) -> Self {
        self.subject = Some(v.into());
        self
    }

    /// Set the explicit message body. Empty strings are treated as unset.
    pub fn message(mut self, v: impl Into<String>) -> Self {
        let s = v.into();
        if !s.is_empty() {
            self.explicit_message = Some(s);
        }
        self
    }

    /// Add a freeform key-value pair that gets formatted into the email body.
    pub fn extra(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.extras.push((key.into(), val.into()));
        self
    }

    /// Construct and validate the ContactForm.
    pub fn build(self) -> Result<ContactForm, ValidationErrors> {
        let message = self.assemble_message();

        let form = ContactForm {
            name: self.name,
            email: self.email,
            phone: self.phone,
            subject: self.subject,
            message,
        };

        form.validate()?;
        Ok(form)
    }

    fn assemble_message(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if !self.extras.is_empty() {
            let extras_str = self
                .extras
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(extras_str);
        }

        if let Some(msg) = &self.explicit_message {
            parts.push(msg.clone());
        }

        parts.join("\n\n")
    }
}

// ── SMTP conversion ────────────────────────────────────────────────────────

#[cfg(feature = "smtp")]
use crate::smtp::EmailData;

#[cfg(feature = "smtp")]
impl From<ContactForm> for EmailData {
    fn from(value: ContactForm) -> Self {
        let ContactForm {
            name,
            email,
            phone,
            subject,
            message,
        } = value;
        EmailData {
            name,
            email,
            phone,
            subject,
            message,
        }
    }
}

// ── API types ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContactResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

// ── Server function ────────────────────────────────────────────────────────

#[server]
pub async fn submit_contact(form: ContactForm) -> Result<ContactResponse, ServerFnError> {
    use http::HeaderMap;
    use tracing::error;
    use tracing::warn;

    use crate::rate_limit;
    #[cfg(feature = "smtp")]
    use crate::smtp;
    use crate::utils::get_client_ip;

    let headers: HeaderMap = dioxus::fullstack::FullstackContext::extract().await?;
    let ip = get_client_ip(&headers);

    // Validation is expected to have happened before calling this function
    // (e.g. via ContactFormBuilder::build).  The assert below catches
    // programming errors in debug builds only.
    #[cfg(debug_assertions)]
    {
        use validator::Validate;
        debug_assert!(
            form.validate().is_ok(),
            "submit_contact received an invalid form; validate before sending"
        );
    }

    if !rate_limit::check_contact_rate_limit(&ip) {
        warn!("Rate limit exceeded for IP {ip}");
        return Ok(ContactResponse {
            success: false,
            message: "Too many requests. Please try again later.".to_string(),
            errors: None,
        });
    }

    #[cfg(feature = "smtp")]
    {
        use tracing::info;

        match smtp::send_email(form).await {
            Ok(()) => {
                info!("Message sent successfully from IP {ip}");
                Ok(ContactResponse {
                    success: true,
                    message: "Message sent successfully!".to_string(),
                    errors: None,
                })
            }
            Err(e) => {
                error!("Failed to send message from IP {ip}: {e}");
                Ok(ContactResponse {
                    success: false,
                    message: "Failed to send message. Please try again later.".to_string(),
                    errors: None,
                })
            }
        }
    }

    #[cfg(not(feature = "smtp"))]
    {
        let _ = (form, ip);
        error!("SMTP feature not enabled");
        Ok(ContactResponse {
            success: false,
            message: "Server email configuration missing.".to_string(),
            errors: None,
        })
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation;

    fn valid_form() -> ContactForm {
        ContactForm {
            name: "John Doe".to_string(),
            email: Some("john@example.com".to_string()),
            phone: None,
            subject: None,
            message: "I would like to discuss a project.".to_string(),
        }
    }

    // ── Struct validation ──────────────────────────────────────────────

    #[test]
    fn valid_form_passes() {
        assert!(valid_form().validate().is_ok());
    }

    #[test]
    fn rejects_email_only_with_invalid_email() {
        let form = ContactForm {
            email: Some("bad".into()),
            phone: Some("08123456789".into()),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn phone_only_without_email_passes() {
        let form = ContactForm {
            email: None,
            phone: Some("08123456789".into()),
            ..valid_form()
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn email_only_without_phone_passes() {
        let form = ContactForm {
            email: Some("user@example.com".into()),
            phone: None,
            ..valid_form()
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn rejects_neither_email_nor_phone() {
        let form = ContactForm {
            email: None,
            phone: None,
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn rejects_name_shorter_than_2() {
        let form = ContactForm {
            name: "J".into(),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn rejects_name_longer_than_100() {
        let form = ContactForm {
            name: "a".repeat(101),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn rejects_message_shorter_than_10() {
        let form = ContactForm {
            message: "ab".into(),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn rejects_message_longer_than_1000() {
        let form = ContactForm {
            message: "a".repeat(1001),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn subject_none_passes() {
        let form = ContactForm {
            subject: None,
            ..valid_form()
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn rejects_subject_shorter_than_2() {
        let form = ContactForm {
            subject: Some("a".into()),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn rejects_subject_longer_than_200() {
        let form = ContactForm {
            subject: Some("a".repeat(201)),
            ..valid_form()
        };
        assert!(form.validate().is_err());
    }

    #[test]
    fn subject_min_length_passes() {
        let form = ContactForm {
            subject: Some("ab".into()),
            ..valid_form()
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn subject_max_length_passes() {
        let form = ContactForm {
            subject: Some("a".repeat(200)),
            ..valid_form()
        };
        assert!(form.validate().is_ok());
    }

    // ── Builder ─────────────────────────────────────────────────────────

    #[test]
    fn builder_produces_valid_form_with_email() {
        let form = ContactForm::builder("Alice")
            .email("alice@example.com")
            .message("Hello world, this is a test message!")
            .build()
            .unwrap();

        assert_eq!(form.name, "Alice");
        assert_eq!(form.email, Some("alice@example.com".into()));
        assert!(form.phone.is_none());
        assert_eq!(form.message, "Hello world, this is a test message!");
    }

    #[test]
    fn builder_produces_valid_form_with_phone() {
        let form = ContactForm::builder("Bob")
            .phone("08123456789")
            .message("Hello world, this is a test message!")
            .build()
            .unwrap();

        assert_eq!(form.name, "Bob");
        assert!(form.email.is_none());
        assert_eq!(form.phone, Some("08123456789".into()));
    }

    #[test]
    fn builder_rejects_no_email_or_phone() {
        let result = ContactForm::builder("Charlie")
            .message("Hello world, this is a test message!")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn builder_rejects_bad_email() {
        let result = ContactForm::builder("Alice")
            .email("not-an-email")
            .phone("08123456789")
            .message("Hello world, this is a test message!")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn builder_assembles_message_from_extras() {
        let form = ContactForm::builder("Parent")
            .email("p@example.com")
            .extra("Ananda", "Budi")
            .extra("Usia/Kelas", "5 tahun")
            .build()
            .unwrap();

        assert_eq!(form.message, "Ananda: Budi\nUsia/Kelas: 5 tahun");
    }

    #[test]
    fn builder_assembles_message_from_extras_and_explicit() {
        let form = ContactForm::builder("Parent")
            .email("p@example.com")
            .extra("Ananda", "Budi")
            .message("Some extra notes")
            .build()
            .unwrap();

        assert_eq!(form.message, "Ananda: Budi\n\nSome extra notes");
    }

    #[test]
    fn builder_defaults_message_when_no_extras_and_empty_message() {
        // message() with empty string is treated as unset
        let result = ContactForm::builder("Parent")
            .email("p@example.com")
            .message("")
            .build();

        // message is empty, which fails length validation
        assert!(result.is_err());
    }

    #[test]
    fn builder_extras_only_meets_message_length_requirement() {
        // Extras alone should produce a message long enough for validation.
        let form = ContactForm::builder("Parent")
            .email("p@example.com")
            .extra("Ananda", "Budi Santoso")
            .extra("Program", "Calistung")
            .build()
            .unwrap();

        assert!(
            form.message.len() >= 10,
            "extras-only message must meet 10-char minimum"
        );
    }

    #[test]
    fn builder_accepts_unicode_email() {
        let form = ContactForm::builder("User")
            .email("user@пример.com")
            .message("Hello everyone, this is a test message.")
            .build()
            .unwrap();

        assert_eq!(form.email, Some("user@пример.com".into()));
    }

    #[test]
    fn builder_phone_without_email_passes_if_extras_sufficient() {
        // Phone-only form with generous extras should pass length validation.
        let form = ContactForm::builder("Charlie")
            .phone("08123456789")
            .extra(
                "Alamat",
                "Kp. Bojongkulur, RT 01/02, Kec. Cileungsi, Kab. Bogor",
            )
            .extra("Catatan", "Prefensi jadwal sore hari setelah maghrib")
            .build()
            .unwrap();

        assert!(form.email.is_none());
        assert_eq!(form.phone, Some("08123456789".into()));
    }

    #[test]
    fn builder_email_opt_override() {
        // Later .email() should override earlier .email_opt().
        let form = ContactForm::builder("Alice")
            .email_opt(Some("old@example.com".into()))
            .email("new@example.com")
            .message("Hello world, this is a test message!")
            .build()
            .unwrap();

        assert_eq!(form.email, Some("new@example.com".into()));
    }

    #[test]
    fn builder_produces_valid_form_with_opt_helpers() {
        let email_val: Option<String> = None;
        let phone_val: Option<String> = Some("08123456789".into());

        let form = ContactForm::builder("Alice")
            .email_opt(email_val)
            .phone_opt(phone_val)
            .message("Hello world, this is a test message!")
            .build()
            .unwrap();

        assert!(form.email.is_none());
        assert_eq!(form.phone, Some("08123456789".into()));
    }

    // ── Serialization ───────────────────────────────────────────────────

    #[test]
    fn serialization_roundtrip() {
        let form = valid_form();
        let json = serde_json::to_string(&form).unwrap();
        let deserialized: ContactForm = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, form.name);
        assert_eq!(deserialized.email, form.email);
        assert_eq!(deserialized.phone, form.phone);
        assert_eq!(deserialized.subject, form.subject);
        assert_eq!(deserialized.message, form.message);
    }

    #[test]
    fn response_omits_errors_when_none() {
        let resp = ContactResponse {
            success: true,
            message: "ok".into(),
            errors: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("errors"),
            "None errors should be skipped by #[serde(skip_serializing_if)]"
        );
    }

    // ── Error formatting ────────────────────────────────────────────────

    #[test]
    fn formats_schema_error_when_no_email_or_phone() {
        let form = ContactForm {
            email: None,
            phone: None,
            ..valid_form()
        };
        let errors = form.validate().unwrap_err();
        let formatted = validation::format_errors(&errors);

        // Schema-level errors must be captured (not just field_errors).
        // This guards against regressions in the format_errors implementation.
        assert!(
            formatted.contains_key("__all__"),
            "schema-level 'contact_required' error should appear under key '__all__': got keys {keys:?}",
            keys = formatted.keys().collect::<Vec<_>>(),
        );
        assert_eq!(
            formatted["__all__"][0],
            "Please provide at least one of email or phone number"
        );
    }

    #[test]
    fn formats_field_errors() {
        let form = ContactForm {
            name: "J".into(),
            email: Some("bad".into()),
            phone: Some("08123456789".into()),
            ..valid_form()
        };
        let errors = form.validate().unwrap_err();
        let formatted = validation::format_errors(&errors);

        assert!(formatted.contains_key("name"));
        assert!(formatted.contains_key("email"));
        assert!(!formatted.contains_key("message"));
    }

    #[test]
    fn formats_empty_errors_returns_empty_map() {
        use validator::ValidationErrors;
        let empty = ValidationErrors::new();
        let formatted = validation::format_errors(&empty);
        assert!(formatted.is_empty());
    }

    #[test]
    fn formats_field_errors_with_correct_messages() {
        let form = ContactForm {
            name: "J".into(),
            email: Some("bad".into()),
            phone: Some("08123456789".into()),
            ..valid_form()
        };
        let errors = form.validate().unwrap_err();
        let formatted = validation::format_errors(&errors);

        assert_eq!(
            formatted.get("name").unwrap()[0],
            "Name must be between 2 and 100 characters"
        );
        assert_eq!(
            formatted.get("email").unwrap()[0],
            "Please enter a valid email address"
        );
    }

    // ── SMTP conversion (feature-gated) ─────────────────────────────────

    #[cfg(feature = "smtp")]
    #[test]
    fn converts_to_email_data() {
        use crate::smtp::EmailData;

        let form = ContactForm {
            name: "Alice".into(),
            email: Some("alice@test.com".into()),
            phone: None,
            subject: Some("Hi".into()),
            message: "Hello there!".into(),
        };
        let data: EmailData = form.into();

        assert_eq!(data.name, "Alice");
        assert_eq!(data.email, Some("alice@test.com".into()));
        assert_eq!(data.phone, None);
        assert_eq!(data.subject, Some("Hi".into()));
        assert_eq!(data.message, "Hello there!");
    }

    #[cfg(feature = "smtp")]
    #[test]
    fn converts_phone_only_form_to_email_data() {
        use crate::smtp::EmailData;

        let form = ContactForm {
            name: "Bob".into(),
            email: None,
            phone: Some("08123456789".into()),
            subject: None,
            message: "No subject here.".into(),
        };
        let data: EmailData = form.into();

        assert_eq!(data.name, "Bob");
        assert!(data.email.is_none());
        assert_eq!(data.phone, Some("08123456789".into()));
    }
}
