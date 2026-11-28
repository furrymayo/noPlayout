//! A minimal blocking HTTP/1.1 client for the engine's own loopback API, used by the app and by
//! tests. Standard library only: the app needs two requests, not an HTTP stack.

use crate::TOKEN_HEADER;
use crate::health::Health;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the engine is not answering on port {port}: {source}")]
    Unreachable {
        port: u16,
        #[source]
        source: std::io::Error,
    },
    #[error("the engine answered with HTTP {0}: {1}")]
    Status(u16, String),
    #[error("the engine sent a reply that could not be read: {0}")]
    Malformed(String),
}

/// `GET /api/v1/health`.
pub fn get_health(port: u16, timeout: Duration) -> Result<Health, ClientError> {
    let body = request(port, "GET", "/api/v1/health", &[], timeout)?;
    serde_json::from_str(&body).map_err(|e| ClientError::Malformed(e.to_string()))
}

/// `POST /api/v1/internal/shutdown` with the launch token.
pub fn post_shutdown(port: u16, token: &str, timeout: Duration) -> Result<(), ClientError> {
    request(
        port,
        "POST",
        "/api/v1/internal/shutdown",
        &[(TOKEN_HEADER, token)],
        timeout,
    )
    .map(|_| ())
}

/// `POST /api/v1/internal/ctl` (dev control) with the launch token; returns the JSON reply.
pub fn post_ctl(
    port: u16,
    token: &str,
    req: &crate::ctl::Ctl,
    timeout: Duration,
) -> Result<String, ClientError> {
    let body = serde_json::to_string(req).map_err(|e| ClientError::Malformed(e.to_string()))?;
    request_body(
        port,
        "POST",
        "/api/v1/internal/ctl",
        &[(TOKEN_HEADER, token), ("content-type", "application/json")],
        body.as_bytes(),
        timeout,
    )
}

fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Result<String, ClientError> {
    request_body(port, method, path, headers, &[], timeout)
}

fn request_body(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    timeout: Duration,
) -> Result<String, ClientError> {
    let unreachable = |source| ClientError::Unreachable { port, source };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout).map_err(unreachable)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(unreachable)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(unreachable)?;
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
         Content-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).map_err(unreachable)?;
    stream.write_all(body).map_err(unreachable)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(unreachable)?;
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<String, ClientError> {
    let malformed = |m: &str| ClientError::Malformed(m.to_owned());
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| malformed("no end of headers"))?;
    let head = std::str::from_utf8(&raw[..split]).map_err(|_| malformed("headers not UTF-8"))?;
    let mut body = raw[split + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or_else(|| malformed("empty reply"))?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| malformed("bad status line"))?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    if chunked {
        body = dechunk(&body).ok_or_else(|| malformed("bad chunked body"))?;
    }
    let body = String::from_utf8(body).map_err(|_| malformed("body not UTF-8"))?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(ClientError::Status(status, body.trim().to_owned()))
    }
}

fn dechunk(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&data[..line_end]).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(data.get(..size)?);
        data = data.get(size + 2..)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_length_and_chunked_replies() {
        let plain = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}";
        assert_eq!(parse_response(plain).unwrap(), "{}");
        let chunked =
            b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\n{\"a\r\n4\r\n\":1}\r\n0\r\n\r\n";
        assert_eq!(parse_response(chunked).unwrap(), "{\"a\":1}");
        let refused = b"HTTP/1.1 403 Forbidden\r\ncontent-length: 4\r\n\r\nnope";
        assert!(matches!(
            parse_response(refused),
            Err(ClientError::Status(403, b)) if b == "nope"
        ));
        assert!(matches!(
            parse_response(b"garbage"),
            Err(ClientError::Malformed(_))
        ));
    }

    #[test]
    fn closed_port_is_unreachable() {
        // Bind and drop to find a port that is very likely closed.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = get_health(port, Duration::from_millis(500)).unwrap_err();
        assert!(matches!(err, ClientError::Unreachable { .. }), "{err}");
    }
}
