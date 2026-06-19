use std::io::Read;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::redirect::Policy;
use reqwest::{Method, Url};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

const DEFAULT_FETCH_TIMEOUT_SECONDS: u64 = 10;
const MAX_TIMEOUT_SECONDS: u64 = 60;
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FetchRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<String>,
    pub body: Option<String>,
    pub extract: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HeaderEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FetchResponse {
    pub url: String,
    pub method: String,
    pub status: u16,
    pub ok: bool,
    pub headers: Vec<HeaderEntry>,
    pub content_type: Option<String>,
    pub body_text: Option<String>,
    pub json_body: Option<Value>,
    pub extracted: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PingResult {
    pub url: String,
    pub reachable: bool,
    pub status: Option<u16>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Error)]
pub enum WebError {
    #[error("invalid url '{input}': {message}")]
    InvalidUrl { input: String, message: String },
    #[error("unsupported url scheme '{scheme}'; only http and https are allowed")]
    UnsupportedUrlScheme { scheme: String },
    #[error("timeout_seconds must be between 1 and {max}; got {value}")]
    InvalidTimeout { value: u64, max: u64 },
    #[error("unsupported HTTP method '{method}'")]
    UnsupportedMethod { method: String },
    #[error("request body exceeds limit of {limit} bytes")]
    RequestBodyTooLarge { limit: usize },
    #[error("response body exceeds limit of {limit} bytes")]
    ResponseBodyTooLarge { limit: usize },
    #[error("HEAD requests do not support a request body")]
    HeadRequestWithBody,
    #[error("invalid header format at index {index}; expected 'Name: Value'")]
    InvalidHeaderFormat { index: usize },
    #[error("invalid header name at index {index}: {message}")]
    InvalidHeaderName { index: usize, message: String },
    #[error("invalid header value at index {index}: {message}")]
    InvalidHeaderValue { index: usize, message: String },
    #[error("failed to build HTTP client: {0}")]
    ClientBuild(#[source] reqwest::Error),
    #[error("request failed: {0}")]
    Request(#[source] reqwest::Error),
    #[error("failed to read response body: {0}")]
    ReadResponse(#[source] std::io::Error),
    #[error("response body is not valid UTF-8 text: {0}")]
    InvalidUtf8(#[source] std::string::FromUtf8Error),
    #[error("failed to parse JSON response body: {0}")]
    InvalidJsonResponse(#[source] serde_json::Error),
    #[error("response header '{name}' is not valid text: {message}")]
    InvalidResponseHeader { name: String, message: String },
    #[error("extract '{extract}' requires a JSON response body")]
    ExtractRequiresJson { extract: String },
    #[error(
        "unsupported extract syntax '{extract}'; use JSON Pointer (/a/b/0) or dotted path (a.b.0)"
    )]
    UnsupportedExtractSyntax { extract: String },
    #[error("extract path '{extract}' was not found in the JSON response")]
    ExtractPathNotFound { extract: String },
}

pub fn fetch(request: FetchRequest) -> Result<FetchResponse, WebError> {
    let method = parse_method(&request.method)?;
    let url = parse_http_url(&request.url)?;
    let body = validate_request_body(request.body, &method)?;
    let headers = parse_request_headers(&request.headers)?;
    let client = build_client(DEFAULT_FETCH_TIMEOUT_SECONDS)?;

    let mut builder = client.request(method.clone(), url).headers(headers);
    if let Some(body_text) = body {
        builder = builder.body(body_text);
    }

    let mut response = builder.send().map_err(WebError::Request)?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|value| value.to_str())
        .transpose()
        .map_err(|error| WebError::InvalidResponseHeader {
            name: CONTENT_TYPE.as_str().to_owned(),
            message: error.to_string(),
        })?
        .map(str::to_owned);
    let response_headers = collect_response_headers(response.headers())?;
    let body_bytes = read_response_body(&mut response)?;
    let body_text = if body_bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8(body_bytes).map_err(WebError::InvalidUtf8)?)
    };
    let json_body = if is_json_content_type(content_type.as_deref()) {
        match body_text.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                Some(serde_json::from_str(text).map_err(WebError::InvalidJsonResponse)?)
            }
            _ => None,
        }
    } else {
        None
    };
    let extracted = match request.extract.as_deref() {
        Some(extract) => {
            let json = json_body
                .as_ref()
                .ok_or_else(|| WebError::ExtractRequiresJson {
                    extract: extract.to_owned(),
                })?;
            Some(extract_json(json, extract)?)
        }
        None => None,
    };

    Ok(FetchResponse {
        url: response.url().to_string(),
        method: method.as_str().to_owned(),
        status: status.as_u16(),
        ok: status.is_success(),
        headers: response_headers,
        content_type,
        body_text,
        json_body,
        extracted,
    })
}

pub fn ping(url: &str, timeout_seconds: u64) -> Result<PingResult, WebError> {
    let url = parse_http_url(url)?;
    let timeout_seconds = normalize_timeout(timeout_seconds)?;
    let client = build_client(timeout_seconds)?;
    let started_at = Instant::now();
    let response = client.get(url).send().map_err(WebError::Request)?;
    let elapsed_ms = started_at.elapsed().as_millis();

    Ok(PingResult {
        url: response.url().to_string(),
        reachable: true,
        status: Some(response.status().as_u16()),
        elapsed_ms,
    })
}

fn normalize_timeout(timeout_seconds: u64) -> Result<u64, WebError> {
    if (1..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        Ok(timeout_seconds)
    } else {
        Err(WebError::InvalidTimeout {
            value: timeout_seconds,
            max: MAX_TIMEOUT_SECONDS,
        })
    }
}

fn build_client(timeout_seconds: u64) -> Result<Client, WebError> {
    let timeout_seconds = normalize_timeout(timeout_seconds)?;
    Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .connect_timeout(Duration::from_secs(timeout_seconds))
        .redirect(Policy::limited(MAX_REDIRECTS))
        .build()
        .map_err(WebError::ClientBuild)
}

fn parse_http_url(input: &str) -> Result<Url, WebError> {
    let url = Url::parse(input).map_err(|source| WebError::InvalidUrl {
        input: input.to_owned(),
        message: source.to_string(),
    })?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(WebError::UnsupportedUrlScheme {
            scheme: scheme.to_owned(),
        }),
    }
}

fn parse_method(input: &str) -> Result<Method, WebError> {
    let normalized = input.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "GET" => Ok(Method::GET),
        "POST" => Ok(Method::POST),
        "PUT" => Ok(Method::PUT),
        "PATCH" => Ok(Method::PATCH),
        "DELETE" => Ok(Method::DELETE),
        "HEAD" => Ok(Method::HEAD),
        _ => Err(WebError::UnsupportedMethod {
            method: input.into(),
        }),
    }
}

fn validate_request_body(
    body: Option<String>,
    method: &Method,
) -> Result<Option<String>, WebError> {
    match body {
        Some(body_text) => {
            if method == Method::HEAD {
                return Err(WebError::HeadRequestWithBody);
            }
            if body_text.len() > MAX_REQUEST_BODY_BYTES {
                return Err(WebError::RequestBodyTooLarge {
                    limit: MAX_REQUEST_BODY_BYTES,
                });
            }
            Ok(Some(body_text))
        }
        None => Ok(None),
    }
}

fn parse_request_headers(headers: &[String]) -> Result<HeaderMap, WebError> {
    let mut header_map = HeaderMap::new();

    for (index, header) in headers.iter().enumerate() {
        let Some((name, value)) = header.split_once(':') else {
            return Err(WebError::InvalidHeaderFormat { index });
        };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() {
            return Err(WebError::InvalidHeaderFormat { index });
        }

        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            WebError::InvalidHeaderName {
                index,
                message: error.to_string(),
            }
        })?;
        let header_value =
            HeaderValue::from_str(value).map_err(|error| WebError::InvalidHeaderValue {
                index,
                message: error.to_string(),
            })?;
        header_map.append(header_name, header_value);
    }

    Ok(header_map)
}

fn collect_response_headers(headers: &HeaderMap) -> Result<Vec<HeaderEntry>, WebError> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = value
                .to_str()
                .map_err(|error| WebError::InvalidResponseHeader {
                    name: name.as_str().to_owned(),
                    message: error.to_string(),
                })?;
            Ok(HeaderEntry {
                name: name.as_str().to_owned(),
                value: value.to_owned(),
            })
        })
        .collect()
}

fn read_response_body(response: &mut Response) -> Result<Vec<u8>, WebError> {
    if let Some(length) = response.content_length() {
        if length > MAX_RESPONSE_BODY_BYTES as u64 {
            return Err(WebError::ResponseBodyTooLarge {
                limit: MAX_RESPONSE_BODY_BYTES,
            });
        }
    }

    let mut reader = response.take((MAX_RESPONSE_BODY_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(WebError::ReadResponse)?;

    if bytes.len() > MAX_RESPONSE_BODY_BYTES {
        return Err(WebError::ResponseBodyTooLarge {
            limit: MAX_RESPONSE_BODY_BYTES,
        });
    }

    Ok(bytes)
}

fn is_json_content_type(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return false;
    };
    let essence = content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    essence.ends_with("/json") || essence.ends_with("+json")
}

fn extract_json(json: &Value, extract: &str) -> Result<Value, WebError> {
    let pointer = normalize_extract_to_pointer(extract)?;
    json.pointer(&pointer)
        .cloned()
        .ok_or_else(|| WebError::ExtractPathNotFound {
            extract: extract.to_owned(),
        })
}

fn normalize_extract_to_pointer(extract: &str) -> Result<String, WebError> {
    if extract.is_empty() {
        return Err(WebError::UnsupportedExtractSyntax {
            extract: extract.to_owned(),
        });
    }

    if extract.starts_with('/') {
        return Ok(extract.to_owned());
    }

    if extract.contains('/') || extract.contains('[') || extract.contains(']') {
        return Err(WebError::UnsupportedExtractSyntax {
            extract: extract.to_owned(),
        });
    }

    let segments = extract.split('.').collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| segment.is_empty())
        || extract.starts_with('.')
        || extract.ends_with('.')
    {
        return Err(WebError::UnsupportedExtractSyntax {
            extract: extract.to_owned(),
        });
    }

    let pointer = segments
        .into_iter()
        .map(|segment| segment.replace('~', "~0").replace('/', "~1"))
        .fold(String::new(), |mut acc, segment| {
            acc.push('/');
            acc.push_str(&segment);
            acc
        });
    Ok(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use serde_json::json;

    #[test]
    fn fetch_parses_json_and_dotted_extract() {
        let body = r#"{"a":{"b":["x","y"]}}"#;
        let (url, requests, handle) = spawn_server(http_response(
            "200 OK",
            &[("Content-Type", "application/json")],
            body,
        ));

        let result = fetch(FetchRequest {
            url,
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: None,
            extract: Some("a.b.0".to_owned()),
        });

        let response = match result {
            Ok(response) => response,
            Err(error) => panic!("fetch should succeed: {error}"),
        };

        assert_eq!(response.status, 200);
        assert!(response.ok);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(response.extracted, Some(json!("x")));

        let request = recv_request(requests);
        assert!(request.starts_with("GET / HTTP/1.1"));
        join_server(handle);
    }

    #[test]
    fn fetch_supports_json_pointer_extract() {
        let body = r#"{"a":{"b":["x","y"]}}"#;
        let (url, _requests, handle) = spawn_server(http_response(
            "200 OK",
            &[("Content-Type", "application/json; charset=utf-8")],
            body,
        ));

        let result = fetch(FetchRequest {
            url,
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: None,
            extract: Some("/a/b/1".to_owned()),
        });

        let response = match result {
            Ok(response) => response,
            Err(error) => panic!("fetch should succeed: {error}"),
        };

        assert_eq!(response.extracted, Some(json!("y")));
        join_server(handle);
    }

    #[test]
    fn fetch_rejects_extract_for_non_json_response() {
        let (url, _requests, handle) = spawn_server(http_response(
            "200 OK",
            &[("Content-Type", "text/plain")],
            "plain text",
        ));

        let result = fetch(FetchRequest {
            url,
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: None,
            extract: Some("a.b".to_owned()),
        });

        match result {
            Err(WebError::ExtractRequiresJson { extract }) => assert_eq!(extract, "a.b"),
            Ok(_) => panic!("fetch should fail for non-json extraction"),
            Err(error) => panic!("unexpected error: {error}"),
        }

        join_server(handle);
    }

    #[test]
    fn fetch_sends_headers_and_body() {
        let (url, requests, handle) = spawn_server(http_response(
            "201 Created",
            &[("Content-Type", "text/plain")],
            "created",
        ));

        let result = fetch(FetchRequest {
            url,
            method: "POST".to_owned(),
            headers: vec!["X-Test: hello".to_owned()],
            body: Some("payload".to_owned()),
            extract: None,
        });

        match result {
            Ok(response) => assert_eq!(response.status, 201),
            Err(error) => panic!("fetch should succeed: {error}"),
        }

        let request = recv_request(requests).to_ascii_lowercase();
        assert!(request.starts_with("post / http/1.1"));
        assert!(request.contains("x-test: hello"));
        assert!(request.ends_with("\r\n\r\npayload"));
        join_server(handle);
    }

    #[test]
    fn ping_returns_status_and_elapsed() {
        let (url, requests, handle) = spawn_server(http_response(
            "204 No Content",
            &[("Content-Type", "text/plain")],
            "",
        ));

        let result = ping(&url, 2);
        let ping = match result {
            Ok(result) => result,
            Err(error) => panic!("ping should succeed: {error}"),
        };

        assert!(ping.reachable);
        assert_eq!(ping.status, Some(204));
        let request = recv_request(requests);
        assert!(request.starts_with("GET / HTTP/1.1"));
        join_server(handle);
    }

    #[test]
    fn fetch_rejects_unsupported_extract_syntax() {
        let (url, _requests, handle) = spawn_server(http_response(
            "200 OK",
            &[("Content-Type", "application/json")],
            r#"{"items":[1,2,3]}"#,
        ));

        let result = fetch(FetchRequest {
            url,
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: None,
            extract: Some("items[0]".to_owned()),
        });

        match result {
            Err(WebError::UnsupportedExtractSyntax { extract }) => {
                assert_eq!(extract, "items[0]")
            }
            Ok(_) => panic!("fetch should reject unsupported extract syntax"),
            Err(error) => panic!("unexpected error: {error}"),
        }

        join_server(handle);
    }

    fn spawn_server(response: String) -> (String, Receiver<String>, JoinHandle<()>) {
        let listener = match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => listener,
            Err(error) => panic!("failed to bind test server: {error}"),
        };
        let address = match listener.local_addr() {
            Ok(address) => address,
            Err(error) => panic!("failed to read test server address: {error}"),
        };
        let (sender, receiver) = mpsc::channel();

        let handle = thread::spawn(move || {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) => panic!("failed to accept test connection: {error}"),
            };
            if let Err(error) = stream.set_read_timeout(Some(Duration::from_secs(2))) {
                panic!("failed to set read timeout: {error}");
            }
            let request = read_http_request(&mut stream);
            if let Err(error) = sender.send(request) {
                panic!("failed to capture request: {error}");
            }
            if let Err(error) = stream.write_all(response.as_bytes()) {
                panic!("failed to write response: {error}");
            }
            if let Err(error) = stream.flush() {
                panic!("failed to flush response: {error}");
            }
        });

        (format!("http://{address}"), receiver, handle)
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut buffer = [0_u8; 1024];
        let mut request = Vec::new();

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    request.extend_from_slice(&buffer[..read]);
                    if request_is_complete(&request) {
                        break;
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
                Err(error) => panic!("failed to read request: {error}"),
            }
        }

        String::from_utf8_lossy(&request).into_owned()
    }

    fn request_is_complete(request: &[u8]) -> bool {
        let Some(header_end) = find_bytes(request, b"\r\n\r\n") else {
            return false;
        };
        let body_start = header_end + 4;
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        });

        request.len() >= body_start + content_length.unwrap_or(0)
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn http_response(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in headers {
            response.push_str(name);
            response.push_str(": ");
            response.push_str(value);
            response.push_str("\r\n");
        }
        response.push_str("\r\n");
        response.push_str(body);
        response
    }

    fn recv_request(receiver: Receiver<String>) -> String {
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(request) => request,
            Err(error) => panic!("failed to receive request: {error}"),
        }
    }

    fn join_server(handle: JoinHandle<()>) {
        match handle.join() {
            Ok(()) => {}
            Err(_) => panic!("test server thread panicked"),
        }
    }
}
