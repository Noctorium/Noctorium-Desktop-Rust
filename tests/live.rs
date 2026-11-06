//! Against the real services, through the real yt-dlp: off unless asked for, since they need the network.
//!
//! ```text
//! cargo test --test live -- --ignored --nocapture
//! ```

use noctorium::domain::ProviderType;
use noctorium::store::Places;
use noctorium::tools::{self, Tool};
use noctorium::ytdlp::YtDlp;

fn backend() -> std::sync::Arc<YtDlp> {
    let path = tools::find(Tool::YtDlp, &Places::system()).expect("yt-dlp must be installed for the live tests");
    println!("yt-dlp: {}", path.display());
    YtDlp::new(Some(path))
}

#[test]
#[ignore]
fn youtube_music_search_finds_songs_with_artists_and_resolves_one() {
    let backend = backend();
    let tracks = backend.search(ProviderType::YoutubeMusic, "daft punk get lucky", 5).unwrap();
    for t in &tracks {
        println!("{} — {} [{:?}] {}", t.title, t.artist_line(), t.duration_ms, t.source_url);
    }
    assert!(!tracks.is_empty());
    assert!(tracks[0].source_url.starts_with("https://music.youtube.com/watch?v="));
    assert!(tracks.iter().all(|t| !noctorium::ytdlp::has_placeholder_artist(t)), "every song has its artist");
    assert!(tracks.iter().all(|t| t.duration_ms.is_some()), "and its length");
    let resolved = backend.resolve_audio(&tracks[0].source_url).unwrap();
    println!("stream: {}… UA {:?}", &resolved.url[..60.min(resolved.url.len())], resolved.user_agent);
    assert!(resolved.url.starts_with("http"));
}

#[test]
#[ignore]
fn soundcloud_search_finds_songs() {
    let tracks = backend().search(ProviderType::Soundcloud, "lofi", 5).unwrap();
    for t in &tracks {
        println!("{} — {} {}", t.title, t.artist_line(), t.source_url);
    }
    assert!(!tracks.is_empty());
}

#[test]
#[ignore]
fn radio_carries_on_from_a_song() {
    let backend = backend();
    let seed = backend.search(ProviderType::YoutubeMusic, "radiohead weird fishes", 1).unwrap().remove(0);
    let radio = backend.radio(&seed, 10).unwrap();
    for t in &radio {
        println!("{} — {}", t.title, t.artist_line());
    }
    assert!(radio.len() >= 5);
    assert!(radio.iter().all(|t| t.id != seed.id));
}

#[test]
#[ignore]
fn lyrics_are_found_for_a_well_known_song() {
    let seed = backend().search(ProviderType::YoutubeMusic, "rick astley never gonna give you up", 1).unwrap().remove(0);
    let lyrics = noctorium::lyrics::fetch(&seed, noctorium::lyrics::Provider::Lrclib).unwrap().expect("LRCLIB has this one");
    println!("{} lines, synced {}: {}", lyrics.lines.len(), lyrics.synced, lyrics.source);
    for outcome in &lyrics.outcomes {
        println!("  {:?}: {:?} {}", outcome.provider, outcome.status, outcome.detail);
    }
    assert!(lyrics.lines.len() > 10);
    assert_eq!(lyrics.provider, noctorium::lyrics::Provider::Lrclib);
    assert_eq!(lyrics.outcomes.len(), 8, "every source answered");
    // Asking for another source that has it gives that one.
    let other = lyrics.outcomes.iter().find(|o| o.provider != noctorium::lyrics::Provider::Lrclib && o.status == noctorium::lyrics::Status::Found);
    if let Some(other) = other {
        let again = noctorium::lyrics::fetch(&seed, other.provider).unwrap().unwrap();
        assert_eq!(again.provider, other.provider);
    }
}

#[test]
#[ignore]
fn mpv_plays_pauses_seeks_and_reports_the_end() {
    use noctorium::mpv::{Engine, EngineEvent, Levels, Status};
    use std::time::{Duration, Instant};
    let places = Places::system();
    let backend = backend();
    let mpv = tools::find(Tool::Mpv, &places).expect("mpv must be installed for this test");
    println!("mpv: {}", mpv.display());
    // SoundCloud by default; NOCTORIUM_LIVE_PROVIDER=ytm for YouTube Music, which needs a current yt-dlp.
    let provider = match std::env::var("NOCTORIUM_LIVE_PROVIDER").as_deref() {
        Ok("ytm") => ProviderType::YoutubeMusic,
        _ => ProviderType::Soundcloud,
    };
    let song = backend.search(provider, "daft punk around the world", 1).unwrap().remove(0);
    println!("song: {} — {} {}", song.title, song.artist_line(), song.source_url);
    let resolved = backend.resolve_audio(&song.source_url).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    // Muted: the position still moves, and nobody near the computer is startled.
    let levels = Levels { volume: 0.05, muted: true, speed: 1.0, looping: false, filters: String::new() };
    let engine = Engine::new(Some(mpv), levels, move |event| {
        let _ = sender.send(event);
    });
    let wait_for = |what: &str, check: &dyn Fn() -> bool| {
        let started = Instant::now();
        while !check() {
            assert!(started.elapsed() < Duration::from_secs(30), "waited too long for {what}");
            std::thread::sleep(Duration::from_millis(100));
        }
        println!("{what} after {:?}", started.elapsed());
    };
    let generation = engine.play(resolved.url.clone(), resolved.user_agent.clone(), 0, false);
    wait_for("playing", &|| engine.state().status == Status::Playing && engine.state().position_ms > 500);
    let state = engine.state();
    println!("position {} of {}, format {:?}", state.position_ms, state.duration_ms, state.format);
    assert!(state.duration_ms > 10_000, "mpv knows how long it is");

    engine.set_paused(true);
    wait_for("paused", &|| engine.state().status == Status::Paused);
    let held = engine.state().position_ms;
    std::thread::sleep(Duration::from_millis(800));
    assert!((engine.state().position_ms - held).abs() < 300, "a paused song stays where it is");

    engine.set_paused(false);
    let near_end = engine.state().duration_ms - 3_000;
    engine.seek(near_end);
    wait_for("seeked", &|| engine.state().position_ms >= near_end - 1_000);
    let ended = receiver.recv_timeout(Duration::from_secs(30)).expect("the end of the song is reported");
    match ended {
        EngineEvent::Ended { generation: g, error } => {
            assert_eq!(g, generation);
            assert!(error.is_none(), "it finished rather than failed: {error:?}");
        }
    }
    assert_eq!(engine.state().status, Status::Idle);
}

#[test]
#[ignore]
fn soundcloud_searches_side_by_side_finish_in_time() {
    let backend = backend();
    let started = std::time::Instant::now();
    let one = backend.search(ProviderType::Soundcloud, "daft punk get lucky", 20);
    println!("one search: {:?} in {:?}", one.as_ref().map(|t| t.len()), started.elapsed());
    let started = std::time::Instant::now();
    let handles: Vec<_> = ["new electronic music", "lofi remix", "daft punk"]
        .into_iter()
        .map(|q| {
            let b = backend.clone();
            std::thread::spawn(move || b.search(ProviderType::Soundcloud, q, 12).map(|t| t.len()))
        })
        .collect();
    for h in handles {
        println!("side by side: {:?}", h.join().unwrap());
    }
    println!("three at once in {:?}", started.elapsed());
}

/// Installs the newest yt-dlp with the installer the Settings page uses, into `NOCTORIUM_RUST_HOME`, and
/// plays a YouTube Music song through it. Downloads about 24 MB from yt-dlp's own GitHub release.
#[test]
#[ignore]
fn the_installer_fetches_yt_dlp_and_youtube_music_plays_through_it() {
    use noctorium::mpv::{Engine, Levels, Status};
    use std::time::{Duration, Instant};
    let places = Places::system();
    assert!(std::env::var_os("NOCTORIUM_RUST_HOME").is_some(), "set NOCTORIUM_RUST_HOME to a test folder");
    let message = tools::install(Tool::YtDlp, &places, &|_| {}).unwrap();
    println!("{message}");
    let path = tools::find(Tool::YtDlp, &places).unwrap();
    println!("installed at {}", path.display());
    assert!(path.starts_with(places.root.as_ref().unwrap()), "the installed copy is the one found");
    let started = Instant::now();
    let version = tools::version(Tool::YtDlp, &path).unwrap();
    println!("version {version}, answered in {:?}", started.elapsed());
    assert!(tools::yt_dlp_age_days(&version).unwrap() < tools::YT_DLP_STALE_DAYS);

    let backend = YtDlp::new(Some(path));
    let song = backend.search(ProviderType::YoutubeMusic, "daft punk around the world", 1).unwrap().remove(0);
    println!("song: {} — {}", song.title, song.artist_line());
    let started = Instant::now();
    let resolved = backend.resolve_audio(&song.source_url).unwrap();
    println!("resolved in {:?}", started.elapsed());
    let mpv = tools::find(Tool::Mpv, &places).expect("mpv must be installed for this test");
    let levels = Levels { volume: 0.05, muted: true, speed: 1.0, looping: false, filters: String::new() };
    let engine = Engine::new(Some(mpv), levels, |_| {});
    engine.play(resolved.url, resolved.user_agent, 0, false);
    let started = Instant::now();
    while !(engine.state().status == Status::Playing && engine.state().position_ms > 1_000) {
        assert!(started.elapsed() < Duration::from_secs(30), "YouTube Music did not start playing");
        std::thread::sleep(Duration::from_millis(100));
    }
    let state = engine.state();
    println!("playing at {} of {} ms, {:?}", state.position_ms, state.duration_ms, state.format);
    engine.stop();
}

/// The same lookup as playing does, from inside the job object Noctorium puts itself in on Windows.
#[test]
#[ignore]
fn youtube_resolves_from_inside_the_job() {
    let places = Places::system();
    let path = tools::find(Tool::YtDlp, &places).unwrap();
    println!("yt-dlp: {}", path.display());
    let backend = YtDlp::new(Some(path));
    let started = std::time::Instant::now();
    let plain = backend.resolve_audio("https://music.youtube.com/watch?v=K0HSD_i2DvA").map(|r| r.url.len());
    println!("outside the job: {plain:?} in {:?}", started.elapsed());
    noctorium::platform::tie_children_to_this_process();
    backend.forget_audio("https://music.youtube.com/watch?v=K0HSD_i2DvA");
    let started = std::time::Instant::now();
    let inside = backend.resolve_audio("https://music.youtube.com/watch?v=K0HSD_i2DvA").map(|r| r.url.len());
    println!("inside the job: {inside:?} in {:?}", started.elapsed());
    assert!(inside.is_ok());
}

/// mpv takes the whole filter chain -- equaliser, evening out, booster with its limiter and clipper -- and
/// plays through it, and takes a new chain while playing.
#[test]
#[ignore]
fn mpv_plays_through_the_equaliser_booster_and_loudness_filters() {
    use noctorium::audio::{Equalizer, EqualizerPreset, Shaping};
    use noctorium::mpv::{Engine, Levels, Status};
    use std::time::{Duration, Instant};
    let places = Places::system();
    let backend = backend();
    let mpv = tools::find(Tool::Mpv, &places).expect("mpv must be installed for this test");
    let song = backend.search(ProviderType::Soundcloud, "lofi", 1).unwrap().remove(0);
    let resolved = backend.resolve_audio(&song.source_url).unwrap();
    let shaping = Shaping {
        equalizer: Equalizer { enabled: true, preset: EqualizerPreset::Rock, ..Equalizer::default() },
        boost_percent: 300,
        volume: 0.05,
        even_loudness: true,
    };
    println!("af: {}", shaping.filters());
    let mut levels = Levels {
        volume: (shaping.mpv_volume() / 100.0) as f32,
        muted: true,
        speed: 1.0,
        looping: false,
        filters: shaping.filters(),
    };
    let engine = Engine::new(Some(mpv), levels.clone(), |event| println!("{event:?}"));
    engine.play(resolved.url, resolved.user_agent, 0, false);
    let started = Instant::now();
    while !(engine.state().status == Status::Playing && engine.state().position_ms > 1_000) {
        assert!(started.elapsed() < Duration::from_secs(30), "mpv did not play through the filters");
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("playing through the filters after {:?}", started.elapsed());
    levels.filters = Shaping { boost_percent: 100, ..shaping }.filters();
    engine.set_levels(levels);
    let held = engine.state().position_ms;
    std::thread::sleep(Duration::from_secs(2));
    assert!(engine.state().position_ms > held, "still playing after the chain changed");
    engine.stop();
}

#[test]
#[ignore]
fn sponsorblock_answers_for_a_well_known_video() {
    // Rick Astley's video, which has been marked for years.
    let segments = noctorium::sponsorblock::segments_for("dQw4w9WgXcQ");
    println!("{segments:?}");
}

/// A song downloaded to play offline, and the same song saved as a tagged MP3 by mpv. Writes into
/// `NOCTORIUM_RUST_HOME`, which should be a test folder.
#[test]
#[ignore]
fn a_song_downloads_and_saves_as_a_tagged_mp3() {
    use noctorium::downloads::{self, ExportFormat};
    let places = Places::system();
    assert!(std::env::var_os("NOCTORIUM_RUST_HOME").is_some(), "set NOCTORIUM_RUST_HOME to a test folder");
    let backend = backend();
    let song = backend.search(ProviderType::YoutubeMusic, "daft punk veridis quo", 1).unwrap().remove(0);
    println!("song: {} — {}", song.title, song.artist_line());
    let last = std::sync::Mutex::new(0.0f32);
    let entry = downloads::download(&backend, &places, &song, &|p| *last.lock().unwrap() = p).unwrap();
    println!("downloaded {} ({} bytes), last progress {}", entry.file, entry.bytes, last.lock().unwrap());
    assert!(entry.bytes > 100_000);

    let folder = places.file("exported").unwrap();
    let mpv = tools::find(Tool::Mpv, &places);
    let saved = downloads::export(&backend, mpv.as_deref(), &song, &folder, ExportFormat::Mp3, &|_| {}).unwrap();
    let bytes = std::fs::read(&saved).unwrap();
    println!("saved {} ({} bytes)", saved.display(), bytes.len());
    assert_eq!(&bytes[..3], b"ID3");
    let tag = &bytes[..downloads::existing_tag_length(&bytes)];
    let title: Vec<u8> = song.title.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    assert!(tag.windows(title.len()).any(|w| w == title.as_slice()), "the title is in the tag");
    assert!(tag.windows(4).any(|w| w == b"APIC"), "and the cover");
}

/// SoundCloud through its own interface: search, related tracks and a stream, each timed.
#[test]
#[ignore]
fn soundcloud_answers_through_its_own_interface() {
    use std::time::Instant;
    let started = Instant::now();
    let found = noctorium::soundcloud::search("daft punk", 5).unwrap();
    println!("search: {} in {:?}", found.len(), started.elapsed());
    for t in &found {
        println!("  {} — {} {:?}", t.title, t.artist_line(), t.duration_ms);
    }
    assert!(!found.is_empty());
    let started = Instant::now();
    let related = noctorium::soundcloud::related(&found[0], 5).unwrap();
    println!("related: {} in {:?}", related.len(), started.elapsed());
    let started = Instant::now();
    let stream = noctorium::soundcloud::stream(&found[0].source_url).unwrap();
    println!("stream in {:?}: {}…", started.elapsed(), &stream[..60.min(stream.len())]);
    assert!(stream.starts_with("http"));
}

/// Last.fm and Libre.fm accept the signature (a token comes back), and ListenBrainz refuses a made-up token
/// in words. Nothing is connected and nothing is sent.
#[test]
#[ignore]
fn scrobblers_answer_the_first_step() {
    use noctorium::scrobble::{self, Site};
    for site in [Site::LastFm, Site::LibreFm] {
        let answer = scrobble::begin_authorization(site);
        println!("{}: {:?}", site.name(), answer.as_ref().map(|(token, url)| (token.len(), url.clone())));
        assert!(answer.is_ok(), "{} accepted the signed request", site.name());
    }
    let refused = scrobble::validate_listenbrainz("not-a-real-token");
    println!("ListenBrainz: {refused:?}");
    assert!(refused.is_err());
}

/// The Noctorium service answers a sign-in it cannot accept in words. No account is made.
#[test]
#[ignore]
fn the_noctorium_service_refuses_a_made_up_sign_in() {
    let answer = noctorium::account::log_in("nobody@example.invalid", "not-a-password");
    println!("{answer:?}");
    assert!(answer.is_err());
}

/// Deezer's charts and a search through its API, and one of its songs found on YouTube Music.
#[test]
#[ignore]
fn a_deezer_song_is_found_on_youtube_music() {
    let (chart, playlists) = noctorium::deezer::chart().unwrap();
    println!("chart: {} songs, {} playlists", chart.len(), playlists.len());
    let song = noctorium::deezer::search("daft punk harder better faster stronger", 1).unwrap().remove(0);
    println!("deezer: {} — {} {:?}", song.title, song.artist_line(), song.duration_ms);
    let backend = backend();
    let store = std::sync::Mutex::new(noctorium::matching::Store::default());
    let matched = noctorium::matching::find(&store, &backend, &song).unwrap();
    println!("matched: {} — {} {} {:?}", matched.title, matched.artist_line(), matched.source_url, matched.duration_ms);
    assert_eq!(matched.provider, ProviderType::YoutubeMusic);
}

/// Bandcamp: search, discover, a release's songs, and a stream played by mpv.
#[test]
#[ignore]
fn bandcamp_searches_and_streams() {
    use noctorium::mpv::{Engine, Levels, Status};
    use std::time::{Duration, Instant};
    let found = noctorium::bandcamp::search("lofi", 5).unwrap();
    for t in &found {
        println!("search: {} — {} {}", t.title, t.artist_line(), t.source_url);
    }
    assert!(!found.is_empty());
    let top = noctorium::bandcamp::discover(false).unwrap();
    println!("discover: {} songs, first {:?}", top.len(), top.first().map(|t| &t.title));
    let started = Instant::now();
    let stream = noctorium::bandcamp::stream(&found[0].source_url).unwrap();
    println!("stream in {:?}: {}…", started.elapsed(), &stream[..50.min(stream.len())]);
    let mpv = tools::find(Tool::Mpv, &Places::system()).expect("mpv");
    let engine = Engine::new(Some(mpv), Levels { volume: 0.05, muted: true, speed: 1.0, looping: false, filters: String::new() }, |_| {});
    engine.play(stream, None, 0, false);
    let started = Instant::now();
    while !(engine.state().status == Status::Playing && engine.state().position_ms > 500) {
        assert!(started.elapsed() < Duration::from_secs(30), "Bandcamp's stream did not play");
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("playing, {:?}", engine.state().format);
    engine.stop();
}

/// A public Spotify playlist read from its embed page, no sign-in, and its first song found on YouTube Music.
#[test]
#[ignore]
fn a_public_spotify_playlist_is_read_and_matched() {
    let playlist = noctorium::spotify::open_link("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M", None).unwrap().unwrap();
    println!("{} by {:?}: {} songs", playlist.title, playlist.owner_name, playlist.tracks.len());
    assert!(!playlist.tracks.is_empty());
    let first = &playlist.tracks[0];
    println!("first: {} — {} {:?}", first.title, first.artist_line(), first.duration_ms);
    let store = std::sync::Mutex::new(noctorium::matching::Store::default());
    let matched = noctorium::matching::find(&store, &backend(), first).unwrap();
    println!("matched: {} — {} {}", matched.title, matched.artist_line(), matched.source_url);
}

/// Apple Music's catalogue through the web player's own token: charts and a search, and a song matched.
#[test]
#[ignore]
fn apple_musics_catalogue_is_read_and_matched() {
    let (songs, playlists) = noctorium::applemusic::charts("us").unwrap();
    println!("charts: {} songs, {} playlists", songs.len(), playlists.len());
    let found = noctorium::applemusic::search("daft punk get lucky", "us", 3).unwrap();
    for t in &found {
        println!("search: {} — {} {:?}", t.title, t.artist_line(), t.duration_ms);
    }
    let store = std::sync::Mutex::new(noctorium::matching::Store::default());
    let matched = noctorium::matching::find(&store, &backend(), &found[0]).unwrap();
    println!("matched: {} — {} {}", matched.title, matched.artist_line(), matched.source_url);
}

/// mpv plays through the visualiser's meter and reports ten bands and both channels.
#[test]
#[ignore]
fn the_visualiser_measures_the_sound() {
    use noctorium::mpv::{Engine, Levels, Status};
    use std::time::{Duration, Instant};
    let backend = backend();
    let song = backend.search(ProviderType::Soundcloud, "daft punk around the world", 1).unwrap().remove(0);
    let resolved = backend.resolve_audio(&song.source_url).unwrap();
    let mpv = tools::find(Tool::Mpv, &Places::system()).expect("mpv");
    let levels = Levels { volume: 0.05, muted: true, speed: 1.0, looping: false, filters: noctorium::audio::spectrum_meter() };
    let engine = Engine::new(Some(mpv), levels, |_| {});
    engine.play(resolved.url, None, 0, false);
    let started = Instant::now();
    loop {
        let state = engine.state();
        if let Some(frame) = state.analysis.as_ref().filter(|a| a.bands.iter().any(|b| *b > 0.0)) {
            println!("after {:?}: bands {:?} rms {:?}", started.elapsed(), frame.bands.iter().map(|b| (b * 100.0) as i32).collect::<Vec<_>>(), frame.rms);
            assert_eq!(frame.bands.len(), 10);
            assert_eq!(frame.rms.len(), 2);
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "no frame from the meter; status {:?}", state.status);
        let _ = Status::Playing;
        std::thread::sleep(Duration::from_millis(100));
    }
    engine.stop();
}

/// The stream widget server answers its page, its feed, and turns a stranger's name away.
#[test]
#[ignore]
fn the_stream_widgets_are_served() {
    use noctorium::widgets::{Preferences, Server};
    let server = Server::start(Preferences { enabled: true, port: 48_517, ..Preferences::default() }).unwrap();
    let page = ureq::get("http://127.0.0.1:48517/widget/vinyl").call().unwrap().into_string().unwrap();
    assert!(page.contains("<script>") && !page.contains("noctorium:widget-script"));
    let feed: serde_json::Value = ureq::get("http://127.0.0.1:48517/api/now-playing").call().unwrap().into_json().unwrap();
    println!("feed: {feed}");
    assert_eq!(feed["state"], "stopped");
    let stranger = ureq::get("http://127.0.0.1:48517/api/now-playing").set("Host", "evil.example").call();
    assert!(matches!(stranger, Err(ureq::Error::Status(403, _))));
    drop(server);
}

/// Two devices on this computer, with one key, find each other by broadcast and one plays the other.
#[test]
#[ignore]
fn two_devices_find_and_command_each_other() {
    use noctorium::connect::{CommandReply, CommandType, Connect, Identity};
    use std::time::{Duration, Instant};
    let a = Connect::start(Identity { id: "device-a".into(), name: "A".into() }, "shared-key".into(), |_| {}).unwrap();
    let b = Connect::start(Identity { id: "device-b".into(), name: "B".into() }, "shared-key".into(), |asked| {
        println!("B was asked: {:?}", asked.command.kind);
        let _ = asked.reply.send(CommandReply { ok: true, message: None, state: None, handover: None });
    })
    .unwrap();
    let started = Instant::now();
    let peer = loop {
        if let Some(peer) = a.peers().into_iter().find(|p| p.id == "device-b") {
            break peer;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "A never heard B");
        std::thread::sleep(Duration::from_millis(200));
    };
    println!("A found {} at {}:{} after {:?}", peer.name, peer.address, peer.port, started.elapsed());
    let reply = a.command(&peer, noctorium::connect::Command::new(CommandType::Play)).unwrap();
    assert!(reply.ok);
    // A device with another key is not trusted.
    let c = Connect::start(Identity { id: "device-c".into(), name: "C".into() }, "other-key".into(), |_| {}).unwrap();
    std::thread::sleep(Duration::from_secs(4));
    assert!(c.peers().is_empty(), "C trusts nobody it cannot verify");
    drop((a, b, c));
}
