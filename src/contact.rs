use std::collections::HashMap;

use dioxus::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use validator::Validate;

#[derive(Debug, Validate, Serialize, Deserialize)]
pub struct ContactForm {
    #[validate(length(
        min = 2,
        max = 100,
        message = "Name must be between 2 and 100 characters"
    ))]
    pub name: String,

    #[validate(email(message = "Please enter a valid email address"))]
    pub email: String,

    #[validate(length(
        min = 10,
        max = 1000,
        message = "Message must be between 10 and 1000 characters"
    ))]
    pub message: String,
}

#[cfg(feature = "smtp")]
use crate::smtp::EmailData;

#[cfg(feature = "smtp")]
impl From<ContactForm> for EmailData {
    fn from(value: ContactForm) -> Self {
        let ContactForm {
            name,
            email,
            message,
        } = value;
        EmailData {
            name,
            email,
            message,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContactResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

#[server]
pub async fn submit_contact(form: ContactForm) -> Result<ContactResponse, ServerFnError> {
    use http::HeaderMap;
    use tracing::error;
    use tracing::warn;
    use validator::Validate;

    use crate::rate_limit;
    #[cfg(feature = "smtp")]
    use crate::smtp;
    use crate::utils::get_client_ip;
    use crate::validation;

    let headers: HeaderMap = dioxus::fullstack::FullstackContext::extract().await?;
    let ip = get_client_ip(&headers);

    if let Err(errors) = form.validate() {
        warn!("Form validation failed for IP {ip}");
        return Ok(ContactResponse {
            success: false,
            message: "Please fix the form errors and try again.".to_string(),
            errors: Some(validation::format_errors(&errors)),
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation;

    fn valid_form() -> ContactForm {
        ContactForm {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            message: "I would like to discuss a project.".to_string(),
        }
    }

    #[test]
    fn valid_form_passes_validation() {
        let form = valid_form();
        let result = form.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_name_shorter_than_2_characters() {
        let form = ContactForm {
            name: "J".to_string(),
            ..valid_form()
        };
        let result = form.validate();
        assert!(result.is_err());
    }

    #[test]
    fn rejects_name_longer_than_100_characters() {
        let form = ContactForm {
            name: "a".repeat(101),
            ..valid_form()
        };
        let result = form.validate();
        assert!(result.is_err());
    }

    #[test]
    fn rejects_message_shorter_than_10_characters() {
        let form = ContactForm {
            message: "ab".to_string(),
            ..valid_form()
        };
        let result = form.validate();
        assert!(result.is_err());
    }

    #[test]
    fn rejects_message_longer_than_1000_characters() {
        let form = ContactForm {
            message: "a".repeat(1001),
            ..valid_form()
        };
        let result = form.validate();
        assert!(result.is_err());
    }

    #[test]
    fn formats_validated_errors_into_field_map() {
        let form = ContactForm {
            name: "J".to_string(),
            email: "bad".to_string(),
            ..valid_form()
        };
        let errors = form.validate().unwrap_err();
        let formatted = validation::format_errors(&errors);

        assert!(
            formatted.contains_key("name"),
            "should include failing field"
        );
        assert!(
            formatted.contains_key("email"),
            "should include another failing field"
        );
        assert!(
            !formatted.contains_key("message"),
            "must not include passing field"
        );
    }

    #[test]
    fn formats_validated_errors_with_correct_messages() {
        let form = ContactForm {
            name: "J".to_string(),
            email: "bad".to_string(),
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
}
