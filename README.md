# Noctorium for the desktop, in Rust

Noctorium — the music player for YouTube Music and SoundCloud — written in Rust: one home, one search, one
library and one queue, on the same two programs the Kotlin desktop uses underneath, **yt-dlp** to read the
services and **mpv** to make the sound, with the window drawn by [egui](https://github.com/emilk/egui), the
same toolkit as [Noctorium-Stats-Desktop](https://github.com/Noctorium/Noctorium-Stats-Desktop).

> Noctorium is an independent third-party client. It is not affiliated with Google, YouTube, SoundCloud,
> Spotify, Bandcamp, VK, Apple, Deezer, TIDAL, Last.fm, ListenBrainz or Discord.

## Installing it

It is released as **Noctorium Rust**, in every [Noctorium release](https://github.com/Noctorium/Noctorium-Installer/releases/latest)
from 0.21.0 on, and the Noctorium installers offer it beside the rest:

```powershell
& ([scriptblock]::Create((irm https://noctorium.vercel.app/install))) --product rust --yes
```

```bash
curl -fsSL https://noctorium.vercel.app/install | sh -s -- --product rust --yes
```

Or by hand: `noctorium-rust-<version>-windows-x64.zip` holds `Noctorium Rust.exe`; the Linux archive the
program, its menu entry and icon; the Mac zips `Noctorium Rust.app`. Its update button (Settings › About and
updates) runs the installer for it once the window has closed.

## What it does

- **Home**: what you played last, your liked songs, and rows of music from YouTube Music and SoundCloud.
- **Search** YouTube Music, SoundCloud and YouTube, or all at once, with the Kotlin players' bangs at the
  start or the end: `!ytm`, `!sc`, `!yt`/`!ytv`, `!all`. A YouTube or SoundCloud link opens the song or playlist.
  YouTube Music is searched through its own web interface, so every row has its artists, album and length.
- **Library**: liked songs, recently played, your own playlists mixing every service, and YouTube and
  SoundCloud playlists followed by their links.
- **Share links**: any playlist as one `noctorium://playlist/1/…` link, the same the Kotlin players make, so a
  playlist sent from the phone opens here; paste one into search, or anywhere, to open it and save a copy.
- **Queue**: play next, add to queue, reorder, shuffle (undone in the order it came), repeat the queue or one
  song (gapless, done by mpv), and **autoplay**: YouTube Music's own radio, or SoundCloud's related tracks.
- **Lyrics** from every source the Kotlin players ask, all at once -- LRCLIB, Better Lyrics, Karalyr, SyncLRC,
  lyrics.ovh, and Musixmatch, Happi and Genius with your own key -- synced line by line; click a line to
  jump to it, switch source above them, and choose the one they open on.
- **Profile and history**: everything played here, by day, to play again; your profile's name; history kept or
  not, and cleared. **Edit details** puts right what a service calls a song, for the queue, lyrics and
  scrobbles, on this computer only.
- **Themes**: every Kotlin theme, colour for colour — Night, Dusk, Day, the Crimsons, Catppuccin, Nord,
  Dracula, Gruvbox, Rosé Pine, Tokyo Night, Solarized, the four Glow themes with Afterglow's light in the
  colour of the cover playing, a theme of your own from six hex colours, and the **Windows 98, Noctorium 98
  and XP skins**: bevelled buttons, their title bars with working caption buttons, Luna's blue.
- **Personalisation**: the accent (the theme's, five of Noctorium's, the cover's colour, or your own), the
  typeface (system, serif, monospace), sharp, soft or round corners, solid or glass surfaces, the controls'
  icons in seven sets, and every look the Kotlin desktop has: twelve player bars (inline, stacked, centred,
  slim, slim left, spotlight, floating, island, a stereo's display, a taskbar, two toolbars) at the bottom or
  the top, twelve seek bars, twelve Now playing layouts (side by side, panel left, centre stage, focus,
  banner, sing along, full cover, split, cover flow, turntable, big type, playlist), the cover rounded,
  square, round or set in a turning record, a colour wash or the cover behind it, bars or a level meter,
  service badges as marks or letters, and the lyrics' size and alignment.
- **Sound**: a ten-band equaliser with its fourteen presets, a volume booster to 1000% behind a limiter and a
  soft clipper, evening out loudness, playback speed, and SponsorBlock skipping the parts of YouTube videos
  that are not the music.
- **Sleep timer**: 15 to 90 minutes, or the end of the song.
- **Downloads**: songs kept to play offline, and songs saved as files to keep -- a 320 kbps MP3 tagged with
  its title, artist, album and cover, or the original audio.
- **Accounts**: your YouTube Music and SoundCloud, signed in from a browser you already use -- their
  playlists, private ones too, your liked songs, YouTube Music's home made for you, and liking a song there.
  Your own YouTube Music playlists can be made (from a song, or a copy of one of your playlists), renamed,
  described, made public, unlisted or private, emptied of songs and deleted from here.
- **Scrobbling** to Last.fm, Libre.fm and ListenBrainz, and the **Noctorium account** with its statistics.
- **Deezer**: search it with `!dz`, its charts, its playlists and albums by link, and your public profile.
  Its songs, like Spotify's, Apple Music's and Tidal's, are matched on YouTube Music to be played.
- **Spotify**: public playlists and albums by link, your library signed in -- playlists, Liked Songs, top
  and recent tracks -- or read from Spotify's data download, and saving songs to your Liked Songs. **Apple Music**: its catalogue and charts, and your library signed
  in from the browser. **Tidal**: signed in with your own Tidal app, its catalogue and your collection
  (beta). **Bandcamp**: search, albums and tracks, played straight from Bandcamp. **VK**: your music, playlists
  and recommendations, signed in from the browser.
- **Connect**: the Noctorium devices on your network, signed with your Noctorium account's key -- send what
  is playing to one, bring it back, or work its controls; name this computer, switch Connect off, or add a
  device by its address on a network that keeps devices apart.
- **Stream widgets**: what is playing, for OBS or Streamlabs, in the Kotlin desktop's seven looks.
- **Keyboard shortcuts** you can change, global ones that work from any program, the media keys, the
  **tray** (closing to it, if you like), and **launching at login**.
- **Discord**: what is playing on your profile, as the Kotlin players show it.
- **Playback**: a song picked on Home or in a search starts a radio or plays the rest of its shelf; autoplay
  from the same service or always YouTube Music's radio, leaving out what was played lately; the sleep timer
  fading out; your plays in your YouTube Music history; Spotify's songs played by Spotify itself (Premium).
- **Browsing**: card size, service badges always, never or when a row mixes services, buttons always or on
  hover, which parts of Home show (and Bandcamp genres there), which pages the sidebar has, which services
  "Everywhere" asks, text size, animations on or off, and the page Noctorium opens on.
- **Diagnostics** check yt-dlp, mpv, storage and the services; **updates** install from the release's
  installer when you say so; a note from the system can **announce each song**.
- **Quick search** over any page with Ctrl+F, the keyboard's **media keys** from any program on Windows, one
  Noctorium at a time, and a note when a newer release is out.
- **Playback tools**: Settings finds yt-dlp and mpv, says how old yt-dlp is, and installs or updates them
  from their projects' own GitHub releases (yt-dlp checked against its published SHA-256 sums).

Keyboard: Space plays and pauses, Ctrl+→/← next and previous, →/← ten seconds, Ctrl+↑/↓ volume, M mutes,
Ctrl+S shuffle, Ctrl+R repeat, Ctrl+F search, Ctrl+L lyrics, `?` lists them all; Settings › Keyboard
changes them. Settings has a search box of its own.

## Your library, shared with the Kotlin desktop

Settings and the library live in their own folder: `%LOCALAPPDATA%\Noctorium Rust` on Windows,
`~/Library/Application Support/Noctorium Rust` on a Mac, `~/.local/share/noctorium-rust` on Linux.
Playlists, liked songs and history are written in the Kotlin players' own JSON, and on the first run the
playlists, pinned songs and recently played of a Kotlin Noctorium on the same computer are read in — read,
never written back, so both can be used side by side. Settings › Your library brings them over again.

## Building and running

```bash
cargo run --release
```

Rust 1.88 or newer. On Windows that needs Visual Studio's C++ build tools (the "Desktop development with C++"
workload). The window is drawn with wgpu — Direct3D 12 on Windows, Metal on a Mac, Vulkan on Linux — and
falls back to OpenGL; `NOCTORIUM_RENDERER=glow` asks for OpenGL straight away.

yt-dlp and mpv are looked for in this order: `NOCTORIUM_YTDLP_PATH` / `NOCTORIUM_MPV_PATH`, this player's
own `bin` folder, the Kotlin desktop's, `PATH`, and the places installers and package managers put them
(`Program Files\MPV Player`, Scoop, Chocolatey, winget; Homebrew and MacPorts on a Mac). On Windows,
Settings installs both; on Linux mpv comes from the distribution.

On Windows, prefer yt-dlp's folder build (what the installer fetches) to the one-file `yt-dlp.exe`: the
one-file build unpacks itself into the temporary folder every time it starts, which was measured at
twenty-five seconds for `--version` against under three.

## Releasing

Noctorium-Installer's `release.yml` builds it with `packaging/package.sh <version> <platform>` -- the
same script Noctorium Stats has -- for `windows-x64`, `linux-x64`, `macos-arm64` and `macos-x64`, with the
release's version built in as `NOCTORIUM_VERSION`, and checks each archive is named and shaped as the
installers expect (`rust.yml` there). So this repository is tagged with each release, as the others are.
`.github/workflows/ci.yml` here runs clippy, the tests and the packaging script on all three systems.

## Testing

```bash
cargo test
```

The tests that need the network and the real programs are off unless asked for:

```bash
cargo test --test live -- --ignored --nocapture
```

`the_installer_fetches_yt_dlp_and_youtube_music_plays_through_it` downloads yt-dlp into
`NOCTORIUM_RUST_HOME`, which should be a test folder.

Pictures of the window, for working on it:

```bash
NOCTORIUM_CAPTURE=shots NOCTORIUM_CAPTURE_PAGES="home,search:daft punk,play,lyrics,queue,settings" cargo run
```

`theme:WINDOWS_XP`, `looks:nowPlaying=STAGE;cover=RECORD` (lists as `playerButtons=SHUFFLE|STOP`) and
`settings:<words>` set the look and search Settings for the pictures after them; `profile`, `diagnose` and
`edit` open the profile, run the diagnostics and open Edit details.

## Not here yet

Liquid glass here is a see-through panel with a rim and a sheen: egui cannot bend what is behind a panel,
so the Kotlin desktop's refraction is not drawn. The 98 and XP skins draw a title band inside the window
rather than replacing the system's own frame.

## Licence

GPL-3.0-only, as the rest of Noctorium.
