//! Bandcamp: albums and tracks from its own interfaces, played as the 128 kbps MP3 stream Bandcamp gives
//! anyone -- Noctorium-Base's `BandcampClient`, read the same way: the site's search, the phone app's
//! release details, and its discover page.
//!
//! A song's address carries what is needed to find its stream again, as the Kotlin players write it:
//! the page, then `#bandcamp-track=<id>&band=<id>`. Stream addresses expire, so they are asked for at play.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use regex::Regex;
use std::sync::OnceLock;
use std::time::Duration;

const SITE: &str = "https://bandcamp.com";
/// The phone app's interface version. Every recent one answers the same for what is read here.
const MOBILE_VERSION: u32 = 24;
/// Any band id, where the real one is not known: Bandcamp asks for one and does not check it.
const ANY_BAND: u64 = 1;
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .set("User-Agent", BROWSER)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| format!("Bandcamp did not answer: {e}"))?
        .into_json()
        .map_err(|_| "Bandcamp sent a reply this could not read.".to_string())
}

fn post_json(url: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    ureq::post(url)
        .set("User-Agent", BROWSER)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(20))
        .send_string(&body.to_string())
        .map_err(|e| format!("Bandcamp did not answer: {e}"))?
        .into_json()
        .map_err(|_| "Bandcamp sent a reply this could not read.".to_string())
}

/// A cover at the size Bandcamp names `16`, 700 pixels.
pub fn artwork(art_id: Option<u64>) -> Option<String> {
    art_id.filter(|id| *id > 0).map(|id| format!("https://f4.bcbits.com/img/a{id:010}_16.jpg"))
}

/// The address a song is kept under: its page, and the ids that find its stream.
pub fn source(page: Option<&str>, track_id: u64, band_id: Option<u64>) -> String {
    let base = page.unwrap_or(SITE).split('#').next().unwrap_or(SITE);
    match band_id.filter(|b| *b > 0) {
        Some(band) => format!("{base}#bandcamp-track={track_id}&band={band}"),
        None => format!("{base}#bandcamp-track={track_id}"),
    }
}

/// The ids in an address written by [`source`]: the track's, and the band's if it was known.
pub fn parse_source(url: &str) -> Option<(u64, Option<u64>)> {
    let fragment = url.split_once('#')?.1;
    let mut track = None;
    let mut band = None;
    for pair in fragment.split('&') {
        match pair.split_once('=') {
            Some(("bandcamp-track", v)) => track = v.parse().ok(),
            Some(("band", v)) => band = v.parse().ok(),
            _ => {}
        }
    }
    Some((track?, band))
}

pub fn is_bandcamp(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("bandcamp.com") || lower.contains("#bandcamp-track=")
}

#[allow(clippy::too_many_arguments)]
fn track(id: u64, title: &str, artist: &str, page: Option<&str>, band: Option<u64>, art: Option<u64>, duration_ms: Option<i64>, album: Option<&Album>) -> Track {
    Track {
        provider: ProviderType::Bandcamp,
        id: id.to_string(),
        title: title.to_string(),
        artists: vec![Artist { id: format!("BANDCAMP:{artist}"), name: artist.to_string(), provider: ProviderType::Bandcamp }],
        album: album.cloned(),
        duration_ms,
        artwork_url: artwork(art).or_else(|| album.and_then(|a| a.artwork_url.clone())),
        source_url: source(page, id, band),
    }
}

/// Songs and albums, for a search.
pub fn search(query: &str, limit: usize) -> Result<Vec<Track>, String> {
    let body = serde_json::json!({"search_text": query.trim(), "search_filter": "t", "full_page": false, "fan_id": null});
    let answer = post_json(&format!("{SITE}/api/bcsearch_public_api/1/autocomplete_elastic"), body)?;
    Ok(answer["auto"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"].as_str() == Some("t"))
        .filter_map(|item| {
            Some(track(
                item["id"].as_u64()?,
                item["name"].as_str()?,
                item["band_name"].as_str().unwrap_or("Bandcamp"),
                item["item_url_path"].as_str(),
                item["band_id"].as_u64(),
                item["art_id"].as_u64(),
                None,
                None,
            ))
        })
        .take(limit)
        .collect())
}

/// A release's songs, by its kind -- `a` for an album, `t` for a track -- and id.
pub fn release(kind: &str, id: u64, band: Option<u64>) -> Result<Playlist, String> {
    let page = get_json(&format!(
        "{SITE}/api/mobile/{MOBILE_VERSION}/tralbum_details?band_id={}&tralbum_type={kind}&tralbum_id={id}",
        band.unwrap_or(ANY_BAND)
    ))?;
    if let Some(message) = page["error_message"].as_str() {
        return Err(format!("Bandcamp: {message}"));
    }
    let band_id = page["band"]["band_id"].as_u64().or(band);
    let band_name = page["band"]["name"].as_str().unwrap_or("Bandcamp").to_string();
    let artist = page["tralbum_artist"].as_str().filter(|a| !a.is_empty()).map(String::from).unwrap_or(band_name.clone());
    let title = page["title"].as_str().unwrap_or("Bandcamp").to_string();
    let url = page["bandcamp_url"].as_str().map(String::from);
    let art = page["art_id"].as_u64();
    let album = Album {
        id: id.to_string(),
        title: title.clone(),
        artists: vec![Artist { id: format!("BANDCAMP:{artist}"), name: artist.clone(), provider: ProviderType::Bandcamp }],
        provider: ProviderType::Bandcamp,
        artwork_url: artwork(art),
    };
    let tracks: Vec<Track> = page["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["is_streamable"].as_bool() != Some(false))
        .filter_map(|t| {
            let page_url = t["track_url"].as_str().map(String::from).or(url.clone());
            Some(track(
                t["track_id"].as_u64()?,
                t["title"].as_str()?,
                t["band_name"].as_str().filter(|b| !b.is_empty()).unwrap_or(&artist),
                page_url.as_deref(),
                band_id,
                art,
                t["duration"].as_f64().map(|s| (s * 1000.0) as i64).filter(|d| *d > 0),
                Some(&album),
            ))
        })
        .collect();
    Ok(Playlist {
        id: format!("{kind}{id}"),
        title,
        provider: ProviderType::Bandcamp,
        owner_name: Some(artist),
        artwork_url: artwork(art),
        track_count: Some(tracks.len() as i64),
        tracks,
        source_url: url,
    })
}

/// Where a song's audio is now: the stream Bandcamp gives anyone, asked for each time since it expires.
pub fn stream(source_url: &str) -> Result<String, String> {
    let (track_id, band) = match parse_source(source_url) {
        Some(ids) => ids,
        None => {
            let (kind, id, band) = page_ids(source_url)?;
            if kind != "t" {
                return Err("That is an album, not one song.".into());
            }
            (id, Some(band))
        }
    };
    let found = get_json(&format!(
        "{SITE}/api/mobile/{MOBILE_VERSION}/tralbum_details?band_id={}&tralbum_type=t&tralbum_id={track_id}",
        band.unwrap_or(ANY_BAND)
    ))?;
    found["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["track_id"].as_u64() == Some(track_id))
        .and_then(|t| t["streaming_url"]["mp3-128"].as_str())
        .map(String::from)
        .ok_or_else(|| "Bandcamp does not stream this song to people who have not bought it.".into())
}

/// The kind, item id and band id a Bandcamp page says it is.
fn page_ids(url: &str) -> Result<(String, u64, u64), String> {
    static PROPERTIES: OnceLock<Regex> = OnceLock::new();
    static BAND: OnceLock<Regex> = OnceLock::new();
    let properties = PROPERTIES.get_or_init(|| Regex::new(r#"<meta name="bc-page-properties" content="([^"]*)""#).unwrap());
    let band = BAND.get_or_init(|| Regex::new(r#"data-band="\{&quot;id&quot;:(\d+)"#).unwrap());
    let page = ureq::get(url.split('#').next().unwrap_or(url))
        .set("User-Agent", BROWSER)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| format!("Bandcamp did not answer: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())?;
    let band_id = band.captures(&page).and_then(|c| c[1].parse().ok()).ok_or("That page is not a Bandcamp release.")?;
    let json = properties
        .captures(&page)
        .map(|c| c[1].replace("&quot;", "\"").replace("&amp;", "&"))
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .ok_or("That page is not a Bandcamp release.")?;
    let kind = match json["item_type"].as_str() {
        Some("a") | Some("album") => "a",
        Some("t") | Some("track") => "t",
        _ => return Err("That page is not an album or a song.".into()),
    };
    let id = json["item_id"].as_u64().ok_or("That page is not a Bandcamp release.")?;
    Ok((kind.to_string(), id, band_id))
}

/// An album or song page, `artist.bandcamp.com/album/...` or `/track/...`, as a playlist.
pub fn open_link(url: &str) -> Option<Result<Playlist, String>> {
    let lower = url.to_lowercase();
    if !(lower.contains(".bandcamp.com/album/") || lower.contains(".bandcamp.com/track/")) {
        return None;
    }
    Some(page_ids(url).and_then(|(kind, id, band)| release(&kind, id, Some(band))))
}

/// Bandcamp's discover page: what sells most, or what is new, as albums with one featured song each.
pub fn discover(newest: bool) -> Result<Vec<Track>, String> {
    discover_in(newest, None)
}

/// The genres Home can have a row of, by Bandcamp's tag and the name shown.
pub const GENRES: [(&str, &str); 16] = [
    ("electronic", "Electronic"),
    ("hip-hop-rap", "Hip-hop and rap"),
    ("rock", "Rock"),
    ("ambient", "Ambient"),
    ("experimental", "Experimental"),
    ("alternative", "Alternative"),
    ("pop", "Pop"),
    ("metal", "Metal"),
    ("punk", "Punk"),
    ("jazz", "Jazz"),
    ("soundtrack", "Soundtrack"),
    ("folk", "Folk"),
    ("r-b-soul", "R&B and soul"),
    ("lo-fi", "Lo-fi"),
    ("classical", "Classical"),
    ("world", "World"),
];

/// Best-sellers, or what is newest, in one genre or across them all.
pub fn discover_in(newest: bool, tag: Option<&str>) -> Result<Vec<Track>, String> {
    let tags: Vec<&str> = tag.into_iter().collect();
    let body = serde_json::json!({
        "category_id": 0, "tag_norm_names": tags, "geoname_id": 0, "slice": if newest { "new" } else { "top" },
        "time_facet_id": null, "cursor": "*", "size": 24, "include_result_types": ["a", "s"],
    });
    let answer = post_json(&format!("{SITE}/api/discover/1/discover_web"), body)?;
    Ok(answer["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let featured = &item["featured_track"];
            let band_name = item["album_artist"].as_str().filter(|a| !a.is_empty()).or(item["band_name"].as_str()).unwrap_or("Bandcamp");
            Some(track(
                featured["id"].as_u64()?,
                featured["title"].as_str()?,
                featured["band_name"].as_str().filter(|b| !b.is_empty()).unwrap_or(band_name),
                item["item_url"].as_str(),
                item["band_id"].as_u64(),
                item["item_image_id"].as_u64().or(item["art_id"].as_u64()),
                featured["duration"].as_f64().map(|s| (s * 1000.0) as i64),
                None,
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_carries_the_ids_back() {
        let url = source(Some("https://band.bandcamp.com/track/song"), 42, Some(7));
        assert_eq!(url, "https://band.bandcamp.com/track/song#bandcamp-track=42&band=7");
        assert_eq!(parse_source(&url), Some((42, Some(7))));
        assert_eq!(parse_source("https://band.bandcamp.com/track/song"), None);
        assert!(is_bandcamp(&url));
    }

    #[test]
    fn covers_are_numbered_with_their_leading_a() {
        assert_eq!(artwork(Some(123)).as_deref(), Some("https://f4.bcbits.com/img/a0000000123_16.jpg"));
        assert_eq!(artwork(Some(0)), None);
    }
}
