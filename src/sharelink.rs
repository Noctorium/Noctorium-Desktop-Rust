//! A playlist in one link, to send to somebody with Noctorium: `noctorium://playlist/1/` and the playlist's
//! songs, as compact JSON, gzipped, in URL-safe base64. Noctorium-Base's `PlaylistShareLink`, so a link made
//! on the phone opens here and one made here opens there.

use crate::domain::{Artist, ProviderType, Track};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const PREFIX: &str = "noctorium://playlist/1/";
const MAX_TRACKS: usize = 500;
const MAX_PAYLOAD_BYTES: u64 = 512 * 1024;

#[derive(Serialize, Deserialize)]
struct WireTrack {
    #[serde(rename = "p")]
    provider: String,
    #[serde(rename = "i")]
    id: String,
    #[serde(rename = "n")]
    title: String,
    #[serde(rename = "a", default, skip_serializing_if = "String::is_empty")]
    artist: String,
    #[serde(rename = "u")]
    url: String,
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    duration_ms: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct WirePlaylist {
    #[serde(rename = "t")]
    title: String,
    #[serde(rename = "x")]
    tracks: Vec<WireTrack>,
}

fn code(provider: ProviderType) -> &'static str {
    match provider {
        ProviderType::YoutubeMusic => "M",
        ProviderType::YoutubeVideo => "Y",
        ProviderType::Soundcloud => "S",
        ProviderType::Spotify => "P",
        ProviderType::Bandcamp => "B",
        ProviderType::Vk => "K",
        ProviderType::AppleMusic => "A",
        ProviderType::Deezer => "D",
        ProviderType::Tidal => "T",
        ProviderType::Local => "L",
        ProviderType::Radio => "R",
    }
}

fn provider(code: &str) -> Option<ProviderType> {
    Some(match code {
        "M" => ProviderType::YoutubeMusic,
        "Y" => ProviderType::YoutubeVideo,
        "S" => ProviderType::Soundcloud,
        "P" => ProviderType::Spotify,
        "B" => ProviderType::Bandcamp,
        "K" => ProviderType::Vk,
        "A" => ProviderType::AppleMusic,
        "D" => ProviderType::Deezer,
        "T" => ProviderType::Tidal,
        "R" => ProviderType::Radio,
        _ => return None,
    })
}

pub fn encode(title: &str, tracks: &[Track]) -> String {
    let payload = WirePlaylist {
        title: title.chars().take(120).collect(),
        tracks: tracks
            .iter()
            .take(MAX_TRACKS)
            .map(|t| WireTrack {
                provider: code(t.provider).into(),
                id: t.id.clone(),
                title: t.title.clone(),
                artist: t.artist_line(),
                url: t.source_url.clone(),
                duration_ms: t.duration_ms,
            })
            .collect(),
    };
    let json = serde_json::to_vec(&payload).unwrap_or_default();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = gzip.write_all(&json);
    let compressed = gzip.finish().unwrap_or_default();
    format!("{PREFIX}{}", crate::oauth::base64url(&compressed))
}

/// Whether `text` is a share link, to be opened as one.
pub fn is_link(text: &str) -> bool {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    cleaned.starts_with(PREFIX) || cleaned.contains("playlist/1/") && cleaned.starts_with("noctorium:")
}

pub fn decode(link: &str) -> Option<(String, Vec<Track>)> {
    let cleaned: String = link.chars().filter(|c| !c.is_whitespace()).collect();
    let body = cleaned.strip_prefix(PREFIX).or_else(|| cleaned.split("playlist/1/").nth(1))?;
    if body.is_empty() {
        return None;
    }
    let compressed = base64url_decode(body)?;
    let mut text = String::new();
    flate2::read::GzDecoder::new(&compressed[..]).take(MAX_PAYLOAD_BYTES).read_to_string(&mut text).ok()?;
    let payload: WirePlaylist = serde_json::from_str(&text).ok()?;
    let tracks: Vec<Track> = payload
        .tracks
        .into_iter()
        .take(MAX_TRACKS)
        .filter_map(|w| {
            if w.id.is_empty() || w.title.is_empty() || !(w.url.starts_with("https://") || w.url.starts_with("http://")) {
                return None;
            }
            let provider = provider(&w.provider)?;
            let artist = if w.artist.is_empty() { provider.display_name().to_string() } else { w.artist };
            Some(Track {
                provider,
                id: w.id,
                title: w.title,
                artists: vec![Artist { id: format!("{}:{artist}", provider.key_name()), name: artist, provider }],
                album: None,
                duration_ms: w.duration_ms,
                artwork_url: None,
                source_url: w.url,
            })
        })
        .collect();
    if tracks.is_empty() {
        return None;
    }
    let title = if payload.title.trim().is_empty() { "Shared playlist".to_string() } else { payload.title };
    Some((title, tracks))
}

fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = text.trim_end_matches('=').bytes().collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        let take = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        for i in 0..take {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            provider: ProviderType::Soundcloud,
            id: id.into(),
            title: format!("Song {id}"),
            artists: vec![Artist { id: "a".into(), name: "Band".into(), provider: ProviderType::Soundcloud }],
            album: None,
            duration_ms: Some(1000),
            artwork_url: None,
            source_url: format!("https://soundcloud.com/band/{id}"),
        }
    }

    #[test]
    fn a_link_carries_the_playlist_there_and_back() {
        let link = encode("Road trip", &[track("1"), track("2")]);
        assert!(link.starts_with(PREFIX));
        assert!(is_link(&link));
        let (title, tracks) = decode(&format!("  {link}\n")).unwrap();
        assert_eq!(title, "Road trip");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].source_url, "https://soundcloud.com/band/2");
        assert_eq!(tracks[0].artist_line(), "Band");
    }

    #[test]
    fn nonsense_is_no_link() {
        assert!(decode("noctorium://playlist/1/!!!").is_none());
        assert!(decode("https://example.com").is_none());
    }
}
