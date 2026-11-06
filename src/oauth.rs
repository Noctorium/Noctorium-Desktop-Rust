//! Signing in with a service's own sign-in page: OAuth with PKCE, the answer arriving at a small server on
//! the computer's loopback address -- the only redirect an installed program can promise. Spotify's and
//! Tidal's, as Noctorium-Base does both.

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// A random PKCE verifier, and its S256 challenge.
pub fn pkce() -> (String, String) {
    use sha2::{Digest, Sha256};
    const SET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let verifier: String = (0..64).map(|_| SET[rand::random_range(0..SET.len())] as char).collect();
    let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn state() -> String {
    (0..16).map(|_| format!("{:x}", rand::random_range(0..16u8))).collect()
}

pub fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (*b as u32) << (16 - 8 * i));
        for i in 0..(chunk.len() + 1) {
            out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// Opens `authorize_url` with `open`, and waits up to five minutes at `127.0.0.1:<port><path>` for the
/// answer carrying `state`. Answers the authorization code.
pub fn wait_for_code(service: &str, authorize_url: &str, port: u16, path: &str, state: &str, open: impl Fn(&str)) -> Result<String, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|_| format!("Port {port} is taken by another program, and {service}'s answer comes there."))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    open(authorize_url);
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    let page = |message: &str| {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><title>Noctorium</title><body style=\"font-family:sans-serif;background:#000;color:#f8f4ff;display:grid;place-items:center;height:90vh\"><p>{message}</p></body>"
        )
    };
    loop {
        if std::time::Instant::now() > deadline {
            return Err(format!("{service}'s sign-in was not finished in time."));
        }
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut line);
        let target = line.split_whitespace().nth(1).unwrap_or("");
        if !target.starts_with(path) {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            continue;
        }
        let query: std::collections::HashMap<String, String> = target
            .split_once('?')
            .map(|(_, q)| q.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect())
            .unwrap_or_default();
        if query.get("state").map(String::as_str) != Some(state) {
            let _ = stream.write_all(page("This answer was not for this sign-in. Try again from Noctorium.").as_bytes());
            continue;
        }
        if let Some(code) = query.get("code") {
            let _ = stream.write_all(page(&format!("Signed in to {service}. You can close this tab and go back to Noctorium.")).as_bytes());
            return Ok(code.clone());
        }
        let _ = stream.write_all(page(&format!("{service}'s sign-in was cancelled.")).as_bytes());
        return Err(format!("{service}'s sign-in was cancelled."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_has_no_padding() {
        assert_eq!(base64url(b"hello"), "aGVsbG8");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn a_verifier_and_its_challenge() {
        let (verifier, challenge) = pkce();
        assert_eq!(verifier.len(), 64);
        assert_eq!(challenge.len(), 43);
    }
}
