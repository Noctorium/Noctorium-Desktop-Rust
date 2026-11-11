//! Radio stations from every country, from Radio Browser (radio-browser.info). Noctorium-Base's
//! `app.noctorium.radio.RadioBrowser`, the same requests and the same answers.
//!
//! Radio Browser is an open, community-kept list of internet radio -- tens of thousands of stations, each with
//! its stream, country, language, tags and a logo -- free to use with no key, run on a few mirrors that share
//! one database. Its rules are short: say who you are in the User-Agent, ask any mirror the list of mirrors,
//! and count a click when a station is played, which is how its popularity ordering works.
//!
//! A station comes back as a [`Track`] of [`ProviderType::Radio`]: its name for a title, where it is from in
//! place of an artist, its logo for artwork, and the stream itself as [`Track::source_url`], which mpv plays as
//! it is. Everything that already handles a track -- the queue, Played lately, the bar at the foot -- handles a
//! station without knowing it is one.

use crate::domain::{Artist, ProviderType, Track};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A country with stations in it, as the radio search's country picker lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioCountry {
    /// ISO 3166-1 alpha-2, upper case: "DE".
    pub code: String,
    pub name: String,
    pub stations: u32,
}

/// One answer from a mirror: its status, 0 for none at all, and its body.
pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

type Http = dyn Fn(&str) -> Reply + Send + Sync;

/// Mirrors that have stood for years, for when the list of them cannot be had.
pub const FALLBACK_MIRRORS: [&str; 3] = ["de1.api.radio-browser.info", "de2.api.radio-browser.info", "fi1.api.radio-browser.info"];

#[derive(Clone)]
pub struct RadioBrowser {
    http: Arc<Http>,
    mirrors: Arc<Mutex<Option<Vec<String>>>>,
    countries: Arc<Mutex<Option<Vec<RadioCountry>>>>,
}

impl Default for RadioBrowser {
    fn default() -> Self {
        RadioBrowser::with_http(|url| {
            let answer = ureq::get(url)
                .set("Accept", "application/json")
                .set("User-Agent", concat!("Noctorium/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(15))
                .call();
            match answer {
                Ok(response) => Reply { status: response.status(), body: response.into_string().unwrap_or_default() },
                Err(ureq::Error::Status(status, response)) => Reply { status, body: response.into_string().unwrap_or_default() },
                Err(_) => Reply { status: 0, body: String::new() },
            }
        })
    }
}

struct Found {
    track: Track,
    clicks: i64,
}

impl RadioBrowser {
    /// A client asking through `http`, so a test can answer for Radio Browser.
    pub fn with_http(http: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Self {
        RadioBrowser { http: Arc::new(http), mirrors: Arc::new(Mutex::new(None)), countries: Arc::new(Mutex::new(None)) }
    }

    /// Stations whose name or tags match `query`, from `country` (an ISO code) or from everywhere; with no
    /// query, the country's most listened-to. Broken streams are left out, and the most popular come first.
    pub fn search(&self, query: &str, country: Option<&str>, limit: usize) -> Result<Vec<Track>, String> {
        let words = query.trim();
        let place = country.filter(|c| !c.trim().is_empty()).map(|c| format!("&countrycode={}", encode(c))).unwrap_or_default();
        let common = format!("hidebroken=true&order=clickcount&reverse=true&limit={limit}{place}");
        let paths: Vec<String> = if words.is_empty() {
            vec![format!("/json/stations/search?{common}")]
        } else {
            // A name is "Radio Paradise"; a tag is "jazz". Either is what somebody may mean by what they typed.
            vec![
                format!("/json/stations/search?name={}&{common}", encode(words)),
                format!("/json/stations/search?tag={}&{common}", encode(&words.to_lowercase())),
            ]
        };
        let answers: Vec<Result<Vec<Found>, String>> = std::thread::scope(|scope| {
            let asks: Vec<_> = paths.iter().map(|path| scope.spawn(move || self.stations(path))).collect();
            asks.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("Radio Browser did not answer.".into()))).collect()
        });
        if answers.iter().all(Result::is_err) {
            return Err(answers.into_iter().find_map(Result::err).unwrap_or_else(|| "Radio Browser did not answer.".into()));
        }
        let mut found: Vec<Found> = Vec::new();
        for answer in answers.into_iter().flatten() {
            for station in answer {
                if !found.iter().any(|f| f.track.id == station.track.id) {
                    found.push(station);
                }
            }
        }
        // Stable, so stations with as many plays keep the order Radio Browser gave them.
        found.sort_by_key(|f| std::cmp::Reverse(f.clicks));
        Ok(found.into_iter().take(limit).map(|f| f.track).collect())
    }

    /// Every country with a working station, by name. Asked once and kept.
    pub fn countries(&self) -> Result<Vec<RadioCountry>, String> {
        if let Some(kept) = self.countries.lock().ok().and_then(|c| c.clone()) {
            return Ok(kept);
        }
        let reply = self.ask("/json/countries?hidebroken=true&order=name")?;
        let list = parse_countries(&reply.body);
        if let Ok(mut kept) = self.countries.lock() {
            *kept = Some(list.clone());
        }
        Ok(list)
    }

    /// Tells Radio Browser a station was played, as it asks; a failure here is nobody's business.
    pub fn played(&self, station: &Track) {
        if station.provider != ProviderType::Radio {
            return;
        }
        let _ = self.ask(&format!("/json/url/{}", encode(&station.id)));
    }

    fn stations(&self, path: &str) -> Result<Vec<Found>, String> {
        let reply = self.ask(path)?;
        let values: Vec<Value> = serde_json::from_str(&reply.body).unwrap_or_default();
        Ok(values
            .iter()
            .filter_map(|o| station(o).map(|track| Found { track, clicks: o.get("clickcount").and_then(Value::as_i64).unwrap_or(0) }))
            .collect())
    }

    /// One mirror after another until one answers, starting with whichever answered last.
    fn ask(&self, path: &str) -> Result<Reply, String> {
        let mut last: Option<u16> = None;
        for mirror in self.mirror_list() {
            let reply = (self.http)(&format!("https://{mirror}{path}"));
            if reply.ok() {
                if let Ok(mut mirrors) = self.mirrors.lock() {
                    let rest: Vec<String> = mirrors.take().unwrap_or_default().into_iter().filter(|m| *m != mirror).collect();
                    *mirrors = Some(std::iter::once(mirror).chain(rest).collect());
                }
                return Ok(reply);
            }
            last = Some(reply.status);
        }
        Err(match last {
            None | Some(0) => "Radio Browser could not be reached. Check the connection and try again.".into(),
            Some(status) => format!("Radio Browser answered {status}. Try again in a moment."),
        })
    }

    fn mirror_list(&self) -> Vec<String> {
        let Ok(mut mirrors) = self.mirrors.lock() else { return FALLBACK_MIRRORS.iter().map(|m| m.to_string()).collect() };
        if let Some(list) = mirrors.as_ref() {
            return list.clone();
        }
        let reply = (self.http)("https://all.api.radio-browser.info/json/servers");
        let mut listed: Vec<String> = Vec::new();
        if reply.ok() {
            let values: Vec<Value> = serde_json::from_str(&reply.body).unwrap_or_default();
            for name in values.iter().filter_map(|v| text(v, "name")) {
                if name.ends_with(".api.radio-browser.info") && !listed.contains(&name) {
                    listed.push(name);
                }
            }
            use rand::seq::SliceRandom;
            listed.shuffle(&mut rand::rng());
        }
        for fallback in FALLBACK_MIRRORS {
            if !listed.iter().any(|m| m == fallback) {
                listed.push(fallback.to_string());
            }
        }
        *mirrors = Some(listed.clone());
        listed
    }
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>().replace('+', "%20")
}

fn text(o: &Value, key: &str) -> Option<String> {
    o.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn parse_countries(body: &str) -> Vec<RadioCountry> {
    let values: Vec<Value> = serde_json::from_str(body).unwrap_or_default();
    let mut list: Vec<RadioCountry> = Vec::new();
    for o in &values {
        let (Some(code), Some(name)) = (text(o, "iso_3166_1"), text(o, "name")) else { continue };
        let code = code.to_uppercase();
        let stations = o.get("stationcount").and_then(Value::as_u64).unwrap_or(0) as u32;
        if stations > 0 && !list.iter().any(|c| c.code == code) {
            list.push(RadioCountry { code, name, stations });
        }
    }
    list.sort_by_key(|c| c.name.to_lowercase());
    list
}

/// A station as a track, or None for one with no name or nothing to play.
pub fn station(o: &Value) -> Option<Track> {
    let id = text(o, "stationuuid")?;
    let name = text(o, "name")?;
    let stream = text(o, "url_resolved").or_else(|| text(o, "url"))?;
    if !stream.starts_with("http://") && !stream.starts_with("https://") {
        return None;
    }
    let country = text(o, "country");
    let code = text(o, "countrycode").map(|c| c.to_uppercase());
    let state = text(o, "state");
    let tags: Vec<String> = text(o, "tags")
        .map(|t| t.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();
    let bitrate = o.get("bitrate").and_then(Value::as_i64).filter(|b| *b > 0);
    // Where it is from, then what it plays, on the one line a song's artist goes in.
    let mut place: Vec<String> = Vec::new();
    for part in [state, country].into_iter().flatten() {
        if !place.contains(&part) {
            place.push(part);
        }
    }
    let line: Vec<String> = [
        Some(place.join(", ")).filter(|s| !s.is_empty()),
        Some(tags.iter().take(3).cloned().collect::<Vec<_>>().join(", ")).filter(|s| !s.is_empty()),
        bitrate.map(|b| format!("{b} kbps")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let line = if line.is_empty() { "Radio".to_string() } else { line.join(" · ") };
    Some(Track {
        provider: ProviderType::Radio,
        id,
        title: name.split_whitespace().collect::<Vec<_>>().join(" "),
        artists: vec![Artist {
            id: format!("radio-country:{}", code.as_deref().unwrap_or("XX")),
            name: line,
            provider: ProviderType::Radio,
        }],
        album: None,
        duration_ms: None,
        artwork_url: text(o, "favicon").filter(|f| f.starts_with("https://") || f.starts_with("http://")),
        source_url: stream,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn station_json(id: &str, name: &str, clicks: i64) -> String {
        format!(
            r#"{{"stationuuid":"{id}","name":"{name}","url":"http://x/{id}","url_resolved":"https://stream/{id}","country":"Germany",
            "countrycode":"de","state":"Berlin","tags":"pop,rock, ,jazz,blues","bitrate":128,"clickcount":{clicks},
            "favicon":"https://logo/{id}.png"}}"#
        )
    }

    #[test]
    fn reads_a_station_as_a_track() {
        let value: Value = serde_json::from_str(&station_json("u1", "  Radio   Eins ", 3)).unwrap();
        let track = station(&value).unwrap();
        assert_eq!(track.provider, ProviderType::Radio);
        assert_eq!(track.title, "Radio Eins");
        assert_eq!(track.source_url, "https://stream/u1");
        assert_eq!(track.artist_line(), "Berlin, Germany · pop, rock, jazz · 128 kbps");
        assert_eq!(track.artists[0].id, "radio-country:DE");
        assert_eq!(track.duration_ms, None);
        assert_eq!(track.queue_key(), "RADIO:u1");
    }

    #[test]
    fn leaves_out_a_station_with_nothing_to_play() {
        let value: Value = serde_json::from_str(r#"{"stationuuid":"a","name":"A","url":"rtsp://x"}"#).unwrap();
        assert!(station(&value).is_none());
        let value: Value = serde_json::from_str(r#"{"stationuuid":"a","url":"http://x"}"#).unwrap();
        assert!(station(&value).is_none());
    }

    #[test]
    fn searches_names_and_tags_and_puts_the_most_played_first() {
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = asked.clone();
        let radio = RadioBrowser::with_http(move |url| {
            log.lock().unwrap().push(url.to_string());
            if url.contains("/json/servers") {
                return Reply { status: 0, body: String::new() };
            }
            let body = if url.contains("name=") {
                format!("[{},{}]", station_json("a", "Jazz FM", 10), station_json("b", "Both", 50))
            } else {
                format!("[{},{}]", station_json("b", "Both", 50), station_json("c", "Smooth", 99))
            };
            Reply { status: 200, body }
        });
        let found = radio.search("Jazz Radio", Some("de"), 80).unwrap();
        let ids: Vec<&str> = found.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        let asked = asked.lock().unwrap();
        assert!(asked.iter().any(|u| u.starts_with("https://de1.api.radio-browser.info/json/stations/search?name=Jazz%20Radio&hidebroken=true&order=clickcount&reverse=true&limit=80&countrycode=de")));
        assert!(asked.iter().any(|u| u.contains("tag=jazz%20radio")));
    }

    #[test]
    fn nothing_typed_asks_for_the_most_listened_to() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let radio = RadioBrowser::with_http(move |url| {
            if url.contains("/json/servers") {
                return Reply { status: 200, body: r#"[{"name":"at1.api.radio-browser.info"},{"name":"evil.example"}]"#.into() };
            }
            counter.fetch_add(1, Ordering::SeqCst);
            assert!(!url.contains("name=") && !url.contains("tag="), "{url}");
            Reply { status: 200, body: format!("[{}]", station_json("a", "A", 1)) }
        });
        assert_eq!(radio.search("  ", None, 10).unwrap().len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let mirrors = radio.mirror_list();
        assert!(mirrors.contains(&"at1.api.radio-browser.info".to_string()));
        assert!(!mirrors.iter().any(|m| m == "evil.example"));
        assert!(mirrors.iter().any(|m| m == FALLBACK_MIRRORS[0]));
    }

    #[test]
    fn tries_the_next_mirror_and_says_why_none_answered() {
        let radio = RadioBrowser::with_http(|url| {
            if url.contains("de1.") || url.contains("/json/servers") {
                Reply { status: 0, body: String::new() }
            } else {
                Reply { status: 503, body: String::new() }
            }
        });
        assert_eq!(radio.search("x", None, 5).unwrap_err(), "Radio Browser answered 503. Try again in a moment.");
        let offline = RadioBrowser::with_http(|_| Reply { status: 0, body: String::new() });
        assert!(offline.countries().unwrap_err().contains("could not be reached"));
    }

    #[test]
    fn lists_countries_by_name_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let radio = RadioBrowser::with_http(move |url| {
            if url.contains("/json/servers") {
                return Reply { status: 0, body: String::new() };
            }
            counter.fetch_add(1, Ordering::SeqCst);
            Reply {
                status: 200,
                body: r#"[{"name":"germany","iso_3166_1":"de","stationcount":5},{"name":"Austria","iso_3166_1":"AT","stationcount":2},
                    {"name":"Nowhere","iso_3166_1":"NW","stationcount":0},{"name":"Deutschland","iso_3166_1":"DE","stationcount":1}]"#
                    .into(),
            }
        });
        let countries = radio.countries().unwrap();
        assert_eq!(countries.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["AT", "DE"]);
        radio.countries().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
