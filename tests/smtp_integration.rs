use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use fazuh_utils::smtp::EmailData;
use fazuh_utils::smtp::config::SmtpConfig;
use fazuh_utils::smtp::config::SmtpSecurity;
use fazuh_utils::smtp::send_email_with_config;
use mailin_embedded::Handler;
use mailin_embedded::Response;
use mailin_embedded::Server;
use mailin_embedded::SslConfig;
use mailin_embedded::response::OK;

/// Handler that captures all received email data.
#[derive(Clone)]
struct CaptureHandler {
    received: Arc<Mutex<Vec<u8>>>,
}

impl Handler for CaptureHandler {
    fn data(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        self.received.lock().unwrap().extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> Response {
        OK
    }
}

/// Find a free TCP port on localhost.
fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn sends_email_to_local_smtp_server() {
    let received: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let port = find_free_port();
    let addr = format!("127.0.0.1:{port}");

    // Start the mailin-embedded server in a background thread.
    let handler = CaptureHandler {
        received: Arc::clone(&received),
    };
    std::thread::spawn(move || {
        let mut server = Server::new(handler);
        server
            .with_name("test.local")
            .with_ssl(SslConfig::None)
            .expect("SslConfig::None should be valid")
            .with_addr(&addr)
            .expect("binding to {addr} should succeed");
        let _ = server.serve();
    });

    // Give the server a moment to start listening.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Configure the SMTP client to point at our local server.
    let config = SmtpConfig {
        smtp_host: "127.0.0.1".into(),
        smtp_port: port,
        smtp_username: String::new(),
        smtp_password: String::new(),
        smtp_from: "Test <test@example.com>".into(),
        smtp_to: "Dest <dest@example.com>".into(),
        smtp_security: SmtpSecurity::Plain,
    };

    let data = EmailData {
        name: "Alice".into(),
        email: Some("alice@test.com".into()),
        phone: Some("08123456789".into()),
        subject: Some("Integration Test".into()),
        message: "Hello from the integration test!".into(),
    };

    send_email_with_config(data, &config)
        .await
        .expect("send_email_with_config should succeed");

    // Allow the server to process the delivery.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the email was received.
    let captured = received.lock().unwrap();
    let body = String::from_utf8_lossy(&captured);

    assert!(
        !captured.is_empty(),
        "server should have received email data"
    );
    assert!(
        body.contains("From: Alice <alice@test.com>"),
        "body should contain the From line: {body}"
    );
    assert!(
        body.contains("Phone: 08123456789"),
        "body should contain the Phone line: {body}"
    );
    assert!(
        body.contains("Subject: Integration Test"),
        "should have Subject header: {body}"
    );
    assert!(
        body.contains("Hello from the integration test!"),
        "should contain the message body: {body}"
    );
}

#[tokio::test]
async fn sends_email_without_email_field() {
    let received: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let port = find_free_port();
    let addr = format!("127.0.0.1:{port}");

    let handler = CaptureHandler {
        received: Arc::clone(&received),
    };
    std::thread::spawn(move || {
        let mut server = Server::new(handler);
        server
            .with_name("test.local")
            .with_ssl(SslConfig::None)
            .expect("SslConfig::None should be valid")
            .with_addr(&addr)
            .expect("binding should succeed");
        let _ = server.serve();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let config = SmtpConfig {
        smtp_host: "127.0.0.1".into(),
        smtp_port: port,
        smtp_username: String::new(),
        smtp_password: String::new(),
        smtp_from: "Test <test@example.com>".into(),
        smtp_to: "Dest <dest@example.com>".into(),
        smtp_security: SmtpSecurity::Plain,
    };

    // No email, only phone
    let data = EmailData {
        name: "Bob".into(),
        email: None,
        phone: Some("08123456789".into()),
        subject: None,
        message: "Message without email, phone only.".into(),
    };

    send_email_with_config(data, &config)
        .await
        .expect("send_email_with_config should succeed");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let captured = received.lock().unwrap();
    let body = String::from_utf8_lossy(&captured);

    assert!(body.contains("Phone: 08123456789"));
    assert!(body.contains("From: Bob <no-reply>"));
    assert!(body.contains("Subject: [fazuh-site] Message from Bob"));
}
