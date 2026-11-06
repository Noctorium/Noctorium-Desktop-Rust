//! A clear copy of an encrypted HLS stream, for mpv: Noctorium-Base's `HlsRelay`. VK serves its songs as HLS
//! with some segments under AES-128 and some not, which the player cannot follow; this serves the same stream
//! from the computer's loopback address with every segment already decrypted.
//!
//! One server, started the first time it is needed. Each stream gets an address under a secret no other
//! program on the computer can guess.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:143.0) Gecko/20100101 Firefox/143.0";
const STREAM_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub uri: String,
    pub iv: Option<[u8; 16]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub uri: String,
    pub duration: String,
    pub key: Option<Key>,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Playlist {
    pub target_duration: String,
    pub media_sequence: u64,
    pub segments: Vec<Segment>,
    pub init: Option<String>,
    pub ended: bool,
}

struct Stream {
    upstream: String,
    created: Instant,
    playlist: Option<Playlist>,
}

struct Relay {
    port: u16,
    secret: String,
    streams: Mutex<HashMap<String, Stream>>,
    keys: Mutex<HashMap<String, Vec<u8>>>,
}

fn token(bytes: usize) -> String {
    (0..bytes).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

fn relay() -> Result<Arc<Relay>, String> {
    static RELAY: OnceLock<Mutex<Option<Arc<Relay>>>> = OnceLock::new();
    let slot = RELAY.get_or_init(|| Mutex::new(None));
    let mut slot = slot.lock().unwrap();
    if let Some(existing) = slot.as_ref() {
        return Ok(existing.clone());
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("The stream relay could not start: {e}"))?;
    let relay = Arc::new(Relay {
        port: listener.local_addr().map_err(|e| e.to_string())?.port(),
        secret: token(18),
        streams: Mutex::new(HashMap::new()),
        keys: Mutex::new(HashMap::new()),
    });
    let serving = relay.clone();
    std::thread::Builder::new()
        .name("hls-relay".into())
        .spawn(move || {
            for connection in listener.incoming().flatten() {
                let relay = serving.clone();
                std::thread::spawn(move || {
                    let _ = serve(&relay, connection);
                });
            }
        })
        .map_err(|e| e.to_string())?;
    *slot = Some(relay.clone());
    Ok(relay)
}

/// The address mpv is given for `upstream`: the same stream, decrypted.
pub fn relay_for(upstream: &str) -> Result<String, String> {
    let relay = relay()?;
    let id = token(9);
    let mut streams = relay.streams.lock().unwrap();
    streams.retain(|_, s| s.created.elapsed() < STREAM_LIFETIME);
    streams.insert(id.clone(), Stream { upstream: upstream.to_string(), created: Instant::now(), playlist: None });
    Ok(format!("http://127.0.0.1:{}/{}/{id}/index.m3u8", relay.port, relay.secret))
}

fn serve(relay: &Relay, mut connection: TcpStream) -> std::io::Result<()> {
    connection.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(connection.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
    }
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("").split('?').next().unwrap_or("");
    let (status, kind, body) = match route(relay, path) {
        Ok(Some(answer)) => (200, answer.0, answer.1),
        Ok(None) => (404, "text/plain", Vec::new()),
        Err(why) => (502, "text/plain", why.into_bytes()),
    };
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Bad Gateway",
    };
    write!(
        connection,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    if method != "HEAD" {
        connection.write_all(&body)?;
    }
    connection.flush()
}

fn route(relay: &Relay, path: &str) -> Result<Option<(&'static str, Vec<u8>)>, String> {
    let pieces: Vec<&str> = path.trim_matches('/').split('/').collect();
    if pieces.len() < 3 || pieces[0] != relay.secret {
        return Ok(None);
    }
    let (upstream, known) = {
        let streams = relay.streams.lock().unwrap();
        let Some(stream) = streams.get(pieces[1]) else { return Ok(None) };
        (stream.upstream.clone(), stream.playlist.clone())
    };
    let playlist = match known {
        Some(p) => p,
        None => {
            let loaded = load(&upstream)?;
            if let Some(stream) = relay.streams.lock().unwrap().get_mut(pieces[1]) {
                stream.playlist = Some(loaded.clone());
            }
            loaded
        }
    };
    match pieces[2] {
        "index.m3u8" => Ok(Some(("application/vnd.apple.mpegurl", rewritten(&playlist).into_bytes()))),
        "init" => match &playlist.init {
            Some(uri) => Ok(Some(("video/mp4", fetch(uri)?))),
            None => Ok(None),
        },
        "s" => {
            let Some(segment) = pieces.get(3).and_then(|i| i.parse::<usize>().ok()).and_then(|i| playlist.segments.get(i)) else {
                return Ok(None);
            };
            let kind = if playlist.init.is_some() { "video/mp4" } else { "video/mp2t" };
            Ok(Some((kind, clear(relay, segment)?)))
        }
        _ => Ok(None),
    }
}

fn rewritten(playlist: &Playlist) -> String {
    let mut out = format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n",
        playlist.target_duration, playlist.media_sequence
    );
    if playlist.init.is_some() {
        out.push_str("#EXT-X-MAP:URI=\"init\"\n");
    }
    for (index, segment) in playlist.segments.iter().enumerate() {
        out.push_str(&format!("#EXTINF:{},\ns/{index}\n", segment.duration));
    }
    if playlist.ended {
        out.push_str("#EXT-X-ENDLIST\n");
    }
    out
}

fn clear(relay: &Relay, segment: &Segment) -> Result<Vec<u8>, String> {
    let bytes = fetch(&segment.uri)?;
    let Some(key) = &segment.key else { return Ok(bytes) };
    let secret = {
        let known = relay.keys.lock().unwrap().get(&key.uri).cloned();
        match known {
            Some(k) => k,
            None => {
                let fetched = fetch(&key.uri)?;
                relay.keys.lock().unwrap().insert(key.uri.clone(), fetched.clone());
                fetched
            }
        }
    };
    decrypt(&bytes, &secret, &key.iv.unwrap_or_else(|| sequence_iv(segment.sequence)))
}

/// AES-128-CBC with PKCS#7 padding, as HLS encrypts.
pub fn decrypt(bytes: &[u8], key: &[u8], iv: &[u8; 16]) -> Result<Vec<u8>, String> {
    let key: [u8; 16] = key.try_into().map_err(|_| "An HLS key that is not sixteen bytes.".to_string())?;
    cbc::Decryptor::<aes::Aes128>::new(&key.into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(bytes)
        .map_err(|_| "A stream part could not be decrypted.".to_string())
}

/// Where a key gives no IV, HLS uses the segment's sequence number, big-endian, in the last eight bytes.
pub fn sequence_iv(sequence: u64) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[8..].copy_from_slice(&sequence.to_be_bytes());
    iv
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .set("User-Agent", AGENT)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("A stream part could not be fetched: {e}"))?;
    let mut bytes = Vec::new();
    response.into_reader().take(64 << 20).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn resolve(base: &str, reference: &str) -> String {
    url::Url::parse(base).and_then(|b| b.join(reference.trim())).map(|u| u.to_string()).unwrap_or_else(|_| reference.trim().to_string())
}

fn load(upstream: &str) -> Result<Playlist, String> {
    let text = String::from_utf8_lossy(&fetch(upstream)?).into_owned();
    if text.contains("#EXT-X-STREAM-INF") {
        let variant = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'))
            .ok_or("An HLS master playlist with no variants.")?;
        return load(&resolve(upstream, variant));
    }
    parse(upstream, &text)
}

fn attribute(attributes: &str, name: &str) -> Option<String> {
    let pattern = regex::Regex::new(&format!(r#"(?:^|,){name}=("([^"]*)"|[^,]*)"#)).ok()?;
    let found = pattern.captures(attributes)?;
    Some(found.get(2).map(|m| m.as_str()).unwrap_or(&found[1]).to_string())
}

pub fn parse(upstream: &str, text: &str) -> Result<Playlist, String> {
    let mut target = "10".to_string();
    let mut sequence = 0u64;
    let mut key: Option<Key> = None;
    let mut duration: Option<String> = None;
    let mut init = None;
    let mut ended = false;
    let mut segments = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(v) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            target = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("#EXT-X-KEY:") {
            let method = attribute(v, "METHOD").unwrap_or_default();
            key = if method.eq_ignore_ascii_case("NONE") || method.is_empty() {
                None
            } else if method.eq_ignore_ascii_case("AES-128") {
                let uri = attribute(v, "URI").ok_or("An HLS key with no address.")?;
                let iv = attribute(v, "IV").and_then(|hex| {
                    let hex = hex.trim_start_matches("0x").trim_start_matches("0X");
                    let padded = format!("{hex:0>32}");
                    let mut iv = [0u8; 16];
                    for (i, chunk) in padded.as_bytes().chunks(2).enumerate().take(16) {
                        iv[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
                    }
                    Some(iv)
                });
                Some(Key { uri: resolve(upstream, &uri), iv })
            } else {
                return Err(format!("HLS encryption {method} is not AES-128."));
            };
        } else if let Some(v) = line.strip_prefix("#EXT-X-MAP:") {
            init = attribute(v, "URI").map(|u| resolve(upstream, &u));
        } else if let Some(v) = line.strip_prefix("#EXTINF:") {
            duration = Some(v.split(',').next().unwrap_or("").trim().to_string());
        } else if line.starts_with("#EXT-X-ENDLIST") {
            ended = true;
        } else if !line.starts_with('#') {
            segments.push(Segment {
                uri: resolve(upstream, line),
                duration: duration.take().unwrap_or_else(|| target.clone()),
                key: key.clone(),
                sequence: sequence + segments.len() as u64,
            });
        }
    }
    Ok(Playlist { target_duration: target, media_sequence: sequence, segments, init, ended })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlist_with_mixed_encryption_is_read() {
        let text = "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXT-X-MEDIA-SEQUENCE:5\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:10.0,\nseg-1.ts\n\
            #EXT-X-KEY:METHOD=AES-128,URI=\"key.pub\",IV=0x0000000000000000000000000000000A\n#EXTINF:9.5,\nseg-2.ts\n\
            #EXT-X-KEY:METHOD=AES-128,URI=\"key.pub\"\n#EXTINF:4,\nseg-3.ts\n#EXT-X-ENDLIST\n";
        let playlist = parse("https://cs.example/p/index.m3u8?x=1", text).unwrap();
        assert_eq!(playlist.segments.len(), 3);
        assert!(playlist.segments[0].key.is_none());
        assert_eq!(playlist.segments[1].uri, "https://cs.example/p/seg-2.ts");
        assert_eq!(playlist.segments[1].key.as_ref().unwrap().iv.unwrap()[15], 10);
        assert_eq!(playlist.segments[2].key.as_ref().unwrap().iv, None);
        assert_eq!(playlist.segments[2].sequence, 7);
        assert!(playlist.ended);
        assert!(rewritten(&playlist).contains("s/2\n"));
    }

    #[test]
    fn decryption_undoes_aes_128_cbc() {
        use aes::cipher::BlockEncryptMut;
        let (key, iv) = ([7u8; 16], sequence_iv(3));
        let sealed = cbc::Encryptor::<aes::Aes128>::new(&key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(b"some audio bytes");
        assert_eq!(decrypt(&sealed, &key, &iv).unwrap(), b"some audio bytes");
    }
}
