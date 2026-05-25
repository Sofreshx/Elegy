//! Notification helpers for toast delivery and outbound webhooks.

use serde::Serialize;

/// Result of a toast notification request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastResult {
    pub title: String,
    pub body: String,
    pub delivered: bool,
    pub platform: String,
}

/// Result of a webhook notification request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResult {
    pub url: String,
    pub status: u16,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
}

/// Errors returned by notification helpers.
#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    /// The requested notification type is unsupported on this platform.
    #[error("{operation} is unsupported on {platform}")]
    Unsupported {
        operation: &'static str,
        platform: String,
    },
    /// Toast delivery failed.
    #[error("toast delivery failed: {0}")]
    Toast(String),
    /// Webhook delivery failed before an HTTP response was received.
    #[error("webhook request failed: {0}")]
    Webhook(String),
}

/// Deliver a toast notification.
pub fn toast(title: &str, body: &str) -> Result<ToastResult, NotifyError> {
    #[cfg(windows)]
    {
        toast_windows(title, body)
    }

    #[cfg(not(windows))]
    {
        let _ = (title, body);
        Err(NotifyError::Unsupported {
            operation: "toast",
            platform: current_platform(),
        })
    }
}

/// Send a webhook notification with an optional payload.
pub fn webhook(url: &str, payload: Option<&str>) -> Result<WebhookResult, NotifyError> {
    use reqwest::blocking::Client;
    use reqwest::header::CONTENT_TYPE;
    use std::time::Duration;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| NotifyError::Webhook(error.to_string()))?;

    let mut request = client.post(url);

    if let Some(payload) = payload {
        request = request.body(payload.to_owned());
        request = if looks_like_json(payload) {
            request.header(CONTENT_TYPE, "application/json")
        } else {
            request.header(CONTENT_TYPE, "text/plain; charset=utf-8")
        };
    } else {
        request = request.body(String::new());
    }

    let response = request
        .send()
        .map_err(|error| NotifyError::Webhook(error.to_string()))?;
    let status = response.status();
    let response_text =
        response
            .text()
            .ok()
            .and_then(|text| if text.is_empty() { None } else { Some(text) });

    Ok(WebhookResult {
        url: url.to_owned(),
        status: status.as_u16(),
        ok: status.is_success(),
        response_text,
    })
}

#[cfg(windows)]
fn toast_windows(title: &str, body: &str) -> Result<ToastResult, NotifyError> {
    use std::process::Command;

    const POWERSHELL_APP_ID: &str =
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

    let xml = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape_xml(title),
        escape_xml(body)
    );
    let script = format!(
        r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml(@'
{xml}
'@)
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{POWERSHELL_APP_ID}').Show($toast)
"#
    );

    let output = Command::new("powershell")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
        .map_err(|error| NotifyError::Toast(format!("failed to launch PowerShell: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("PowerShell exited with status {}", output.status)
        };
        return Err(NotifyError::Toast(detail));
    }

    Ok(ToastResult {
        title: title.to_owned(),
        body: body.to_owned(),
        delivered: true,
        platform: current_platform(),
    })
}

fn current_platform() -> String {
    std::env::consts::OS.to_owned()
}

fn looks_like_json(payload: &str) -> bool {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return false;
    }

    matches!(trimmed, "true" | "false" | "null")
        || matches!(
            (trimmed.chars().next(), trimmed.chars().last()),
            (Some('{'), Some('}')) | (Some('['), Some(']')) | (Some('"'), Some('"'))
        )
        || trimmed.parse::<i64>().is_ok()
        || trimmed.parse::<f64>().is_ok()
}

#[cfg(windows)]
fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[derive(Debug)]
    struct CapturedRequest {
        request_line: String,
        headers: HashMap<String, String>,
        body: String,
    }

    #[test]
    fn looks_like_json_distinguishes_json_from_text() {
        assert!(looks_like_json(r#"{"event":"notify"}"#));
        assert!(looks_like_json("[1,2,3]"));
        assert!(looks_like_json("\"hello\""));
        assert!(!looks_like_json("plain text"));
        assert!(!looks_like_json(""));
    }

    #[test]
    fn webhook_posts_json_payload_and_returns_metadata() {
        let (base_url, requests, handle) = spawn_test_server(202, "Accepted", "queued");
        let target = format!("{base_url}/notify");

        let result = webhook(&target, Some(r#"{"event":"notify"}"#))
            .expect("webhook with JSON payload should succeed");
        let captured = requests
            .recv_timeout(Duration::from_secs(2))
            .expect("server should capture request");

        assert_eq!(result.url, target);
        assert_eq!(result.status, 202);
        assert!(result.ok);
        assert_eq!(result.response_text.as_deref(), Some("queued"));
        assert_eq!(captured.request_line, "POST /notify HTTP/1.1");
        assert_eq!(captured.body, r#"{"event":"notify"}"#);
        assert_eq!(
            captured.headers.get("content-type").map(String::as_str),
            Some("application/json")
        );

        handle.join().expect("server thread should join cleanly");
    }

    #[test]
    fn webhook_posts_empty_body_when_payload_is_missing() {
        let (base_url, requests, handle) = spawn_test_server(204, "No Content", "");
        let target = format!("{base_url}/empty");

        let result = webhook(&target, None).expect("webhook without payload should succeed");
        let captured = requests
            .recv_timeout(Duration::from_secs(2))
            .expect("server should capture request");

        assert_eq!(result.status, 204);
        assert!(result.ok);
        assert_eq!(result.response_text, None);
        assert_eq!(captured.request_line, "POST /empty HTTP/1.1");
        assert!(captured.body.is_empty());

        handle.join().expect("server thread should join cleanly");
    }

    #[test]
    fn webhook_posts_text_payload_as_text_plain() {
        let (base_url, requests, handle) = spawn_test_server(200, "OK", "sent");
        let target = format!("{base_url}/text");

        let result =
            webhook(&target, Some("plain text")).expect("webhook with text payload should succeed");
        let captured = requests
            .recv_timeout(Duration::from_secs(2))
            .expect("server should capture request");

        assert_eq!(result.status, 200);
        assert!(result.ok);
        assert_eq!(captured.request_line, "POST /text HTTP/1.1");
        assert_eq!(captured.body, "plain text");
        assert_eq!(
            captured.headers.get("content-type").map(String::as_str),
            Some("text/plain; charset=utf-8")
        );

        handle.join().expect("server thread should join cleanly");
    }

    #[cfg(not(windows))]
    #[test]
    fn toast_returns_unsupported_on_non_windows() {
        let error = toast("title", "body").expect_err("toast should be unsupported");

        assert!(matches!(
            error,
            NotifyError::Unsupported {
                operation: "toast",
                ..
            }
        ));
        assert!(error.to_string().contains(std::env::consts::OS));
    }

    fn spawn_test_server(
        status: u16,
        reason: &str,
        response_body: &str,
    ) -> (
        String,
        mpsc::Receiver<CapturedRequest>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose local address");
        let response_body = response_body.to_owned();
        let reason = reason.to_owned();
        let (sender, receiver) = mpsc::channel();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("server should accept connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("server should set read timeout");

            let raw_request = read_request(&mut stream);
            let captured = parse_request(&raw_request);
            sender
                .send(captured)
                .expect("server should send captured request");

            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("server should write response");
            stream.flush().expect("server should flush response");
        });

        (format!("http://{address}"), receiver, handle)
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];

        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    if let Some(header_end) = find_header_end(&buffer) {
                        let content_length = parse_content_length(&buffer[..header_end]);
                        let body_start = header_end + 4;
                        if buffer.len() >= body_start + content_length {
                            break;
                        }
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(error) => panic!("server should read request: {error}"),
            }
        }

        buffer
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn parse_content_length(headers: &[u8]) -> usize {
        let text = String::from_utf8_lossy(headers);
        for line in text.lines() {
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    if let Ok(length) = value.trim().parse::<usize>() {
                        return length;
                    }
                }
            }
        }
        0
    }

    fn parse_request(raw_request: &[u8]) -> CapturedRequest {
        let request_text = String::from_utf8_lossy(raw_request);
        let (head, body) = request_text
            .split_once("\r\n\r\n")
            .expect("request should contain header separator");
        let mut lines = head.lines();
        let request_line = lines
            .next()
            .expect("request should contain request line")
            .to_owned();
        let headers = lines
            .filter_map(|line| {
                line.split_once(':').map(|(name, value)| {
                    (name.trim().to_ascii_lowercase(), value.trim().to_owned())
                })
            })
            .collect();

        CapturedRequest {
            request_line,
            headers,
            body: body.to_owned(),
        }
    }
}
