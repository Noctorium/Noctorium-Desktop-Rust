//! Every service, behind the same three questions: search it, open a link to it, and say whether a link is
//! its. The window asks here and never a service by name, as Noctorium-Base's providers are combined.

use crate::bangs::SearchMode;
use crate::domain::{Playlist, ProviderType, Track};
use crate::ytdlp::YtDlp;

/// What every player says above a service in beta, Noctorium-Base's `BETA_WARNING`.
pub const BETA_WARNING: &str = "In beta: this service may not work as it should, and there is a small chance it gets your account blocked. Use it at your own risk.";

/// The sessions of the services read through their own interfaces, where search and links can reach them:
/// Apple Music's user token and storefront, and Tidal's session.
#[derive(Default)]
pub struct Sessions {
    pub apple_token: Option<String>,
    pub apple_storefront: String,
    pub tidal: Option<crate::tidal::Session>,
}

pub fn sessions() -> std::sync::MutexGuard<'static, Sessions> {
    static SESSIONS: std::sync::OnceLock<std::sync::Mutex<Sessions>> = std::sync::OnceLock::new();
    SESSIONS
        .get_or_init(|| std::sync::Mutex::new(Sessions { apple_storefront: "us".into(), ..Default::default() }))
        .lock()
        .unwrap()
}

/// The service a search mode reaches.
pub fn provider_for(mode: SearchMode) -> Vec<ProviderType> {
    match mode {
        SearchMode::YoutubeMusic => vec![ProviderType::YoutubeMusic],
        SearchMode::YoutubeVideo => vec![ProviderType::YoutubeVideo],
        SearchMode::Soundcloud => vec![ProviderType::Soundcloud],
        SearchMode::Deezer => vec![ProviderType::Deezer],
        SearchMode::Bandcamp => vec![ProviderType::Bandcamp],
        SearchMode::AppleMusic => vec![ProviderType::AppleMusic],
        SearchMode::Spotify => vec![ProviderType::Spotify],
        SearchMode::Tidal => vec![ProviderType::Tidal],
        SearchMode::Vk => vec![ProviderType::Vk],
        SearchMode::Hybrid => vec![ProviderType::YoutubeMusic, ProviderType::Soundcloud],
    }
}

pub fn search(backend: &YtDlp, provider: ProviderType, query: &str, limit: usize) -> Result<Vec<Track>, String> {
    match provider {
        ProviderType::Deezer => crate::deezer::search(query, limit),
        ProviderType::Bandcamp => crate::bandcamp::search(query, limit),
        ProviderType::AppleMusic => {
            let storefront = sessions().apple_storefront.clone();
            crate::applemusic::search(query, &storefront, limit)
        }
        ProviderType::Tidal => {
            let session = sessions().tidal.clone().ok_or("Searching Tidal needs your Tidal app: Services › Tidal.")?;
            session.search(query, limit)
        }
        ProviderType::Vk => crate::vk::current().ok_or("Searching VK needs its sign-in: Services › VK.")?.search(query, limit),
        ProviderType::Spotify => crate::spotify::current()
            .ok_or("Searching Spotify needs its sign-in: Services › Spotify.")?
            .search(query, limit),
        _ => backend.search(provider, query, limit),
    }
}

/// A playlist or an album on a service that is not read through yt-dlp, by its link. None when the link is
/// none of theirs.
pub fn open_link(url: &str) -> Option<Result<Playlist, String>> {
    crate::deezer::open_link(url)
        .or_else(|| crate::bandcamp::open_link(url))
        .or_else(|| crate::spotify::open_link(url, crate::spotify::current().as_ref()))
        .or_else(|| {
            let token = sessions().apple_token.clone();
            crate::applemusic::open_link(url, token.as_deref())
        })
        .or_else(|| {
            let (owner, id, key) = crate::vk::playlist_ids(url)?;
            Some(match crate::vk::current() {
                None => Err("Opening a VK playlist needs its sign-in: Services › VK.".into()),
                Some(session) => session.playlist(owner, id, key.as_deref()).map(|tracks| Playlist {
                    id: format!("{owner}_{id}"),
                    title: "VK playlist".into(),
                    provider: ProviderType::Vk,
                    owner_name: None,
                    artwork_url: tracks.first().and_then(|t| t.artwork_url.clone()),
                    track_count: Some(tracks.len() as i64),
                    tracks,
                    source_url: Some(url.to_string()),
                }),
            })
        })
        .or_else(|| {
            let id = crate::tidal::playlist_id(url)?;
            let session = sessions().tidal.clone();
            Some(match session {
                None => Err("Opening a Tidal playlist needs your Tidal app: Services › Tidal.".into()),
                Some(session) => session.playlist(&id).map(|tracks| Playlist {
                    id: id.clone(),
                    title: "Tidal playlist".into(),
                    provider: ProviderType::Tidal,
                    owner_name: None,
                    artwork_url: None,
                    track_count: Some(tracks.len() as i64),
                    tracks,
                    source_url: Some(url.to_string()),
                }),
            })
        })
}

/// Whether Noctorium can open `url` at all.
pub fn recognises(url: &str) -> bool {
    crate::ytdlp::provider_of(url).is_some() || url.to_lowercase().contains("deezer.com/") || crate::bandcamp::is_bandcamp(url) || crate::spotify::is_spotify(url)
        || crate::applemusic::is_apple(url)
        || crate::tidal::playlist_id(url).is_some()
        || crate::vk::playlist_ids(url).is_some()
}
