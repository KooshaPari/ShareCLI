// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Tray dashboard HTTP client with W3C `traceparent` injection (C05 L44 / T-610).
//!
//! Used by `sharecli-ffi` and integration tests when the desktop tray probes
//! `sharecli serve` over loopback HTTP.
#![allow(dead_code)] // The binary copy only serves HTML; FFI consumes `get` from the library.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::otel::traceparent_http_value;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// GET `url` over HTTP/1.1 with operator/OTel `traceparent` when configured.
pub fn get(url: &str) -> Result<String> {
    let (host, port, path) = parse_http_url(url)?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .with_context(|| format!("resolve {host}:{port}"))?
        .next()
        .with_context(|| format!("no address for {host}:{port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .with_context(|| format!("connect to {host}:{port}"))?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).context("set HTTP read timeout")?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).context("set HTTP write timeout")?;
    let host_header = http_host_header(&host, port);
    let mut req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nConnection: close\r\nAccept: */*\r\n"
    );
    if let Some(tp) = traceparent_http_value() {
        req.push_str(&format!("traceparent: {tp}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).context("write HTTP request")?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).context("read HTTP response")?;
    extract_http_body(&buf)
}

fn http_host_header(host: &str, port: u16) -> String {
    if port == 80 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

fn parse_http_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url.strip_prefix("http://").context("only http:// URLs are supported")?;
    let (authority, path) = match rest.split_once('/') {
        Some((auth, tail)) => (auth, format!("/{tail}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().context("invalid port")?),
        None => (authority.to_string(), 80),
    };
    Ok((host, port, path))
}

fn extract_http_body(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw).context("response not UTF-8")?;
    let (_, body) = text.split_once("\r\n\r\n").context("malformed HTTP response")?;
    Ok(body.to_string())
}

/// Inject `data-traceparent` on the dashboard root element when configured.
pub fn inject_dashboard_traceparent(html: &str) -> String {
    let Some(tp) = traceparent_http_value() else {
        return html.to_string();
    };
    let escaped = tp.replace('&', "&amp;").replace('"', "&quot;").replace('\'', "&#39;");
    html.replace(
        "<html lang=\"en\">",
        &format!("<html lang=\"en\" data-traceparent=\"{escaped}\">"),
    )
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::otel::TEST_ENV_LOCK;

    #[test]
    fn inject_dashboard_traceparent_adds_data_attribute() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        // On Windows env keys are case-insensitive: remove before set so we do
        // not clear the value we just wrote via remove_var("TRACEPARENT").
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01");
        let html = inject_dashboard_traceparent("<html lang=\"en\"><body></body></html>");
        if let Some(previous) = prev {
            std::env::set_var(key, previous);
        } else {
            std::env::remove_var(key);
        }
        if let Some(previous) = prev_upper {
            std::env::set_var("TRACEPARENT", previous);
        }
        assert!(html.contains(
            "data-traceparent=\"00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\""
        ));
    }

    #[test]
    fn get_sends_traceparent_header_and_host_with_port() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            tx.send((request, headers)).unwrap();
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
            stream.write_all(response.as_bytes()).unwrap();
        });

        let key = "traceparent";
        let prev = std::env::var(key).ok();
        let prev_upper = std::env::var("TRACEPARENT").ok();
        // Windows: remove before set (case-insensitive env slot).
        std::env::remove_var("TRACEPARENT");
        std::env::set_var(key, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");

        let url = format!("http://{addr}/healthz");
        let body = get(&url).expect("tray HTTP get");

        if let Some(previous) = prev {
            std::env::set_var(key, previous);
        } else {
            std::env::remove_var(key);
        }
        if let Some(previous) = prev_upper {
            std::env::set_var("TRACEPARENT", previous);
        }

        let (request, headers) = rx.recv_timeout(Duration::from_secs(5)).expect("request captured");
        handle.join().expect("server thread");
        assert_eq!(body, "ok");
        assert!(request.starts_with("GET /healthz HTTP/1.1"));
        assert!(
            headers.contains(&format!("Host: 127.0.0.1:{}\r\n", addr.port())),
            "Host header must include non-default port; got:\n{headers}"
        );
        assert!(headers
            .contains("traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"));
    }

    #[test]
    fn http_host_header_omits_default_port() {
        assert_eq!(http_host_header("127.0.0.1", 80), "127.0.0.1");
        assert_eq!(http_host_header("127.0.0.1", 49152), "127.0.0.1:49152");
    }

    #[test]
    fn parse_http_url_defaults_port() {
        let (host, port, path) = parse_http_url("http://127.0.0.1/healthz").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 80);
        assert_eq!(path, "/healthz");
    }

    #[test]
    fn parse_http_url_rejects_https() {
        assert!(parse_http_url("https://127.0.0.1:9000/").is_err());
    }
}
