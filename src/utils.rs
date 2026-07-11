use http::HeaderMap;

/// Extracts the client IP from request headers.
///
/// Relies on `X-Forwarded-For` (first address) with fallback to `X-Real-Ip`.
pub fn get_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("X-Real-Ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;
    use http::HeaderValue;

    use super::*;

    #[test]
    fn extracts_ip_from_x_forwarded_for_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("1.2.3.4, 5.6.7.8"),
        );

        let result = get_client_ip(&headers);

        assert_eq!(result, "1.2.3.4");
    }

    #[test]
    fn extracts_ip_from_x_real_ip_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-Ip", HeaderValue::from_static("9.8.7.6"));

        let result = get_client_ip(&headers);

        assert_eq!(result, "9.8.7.6");
    }

    #[test]
    fn prefers_x_forwarded_for_over_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("1.1.1.1"));
        headers.insert("X-Real-Ip", HeaderValue::from_static("2.2.2.2"));

        let result = get_client_ip(&headers);

        assert_eq!(result, "1.1.1.1");
    }

    #[test]
    fn returns_unknown_when_no_ip_headers_present() {
        let headers = HeaderMap::new();

        let result = get_client_ip(&headers);

        assert_eq!(result, "unknown");
    }

    #[test]
    fn trims_whitespace_from_ip_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("  192.168.1.1  "),
        );

        let result = get_client_ip(&headers);

        assert_eq!(result, "192.168.1.1");
    }
}
