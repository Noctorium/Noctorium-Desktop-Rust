//! What Noctorium does to the sound on its way out: the equaliser, the volume booster, and evening out how
//! loud one song is against the next. All of it as mpv audio filters, as Noctorium-Base's `MpvFilters` has it.

use serde::{Deserialize, Serialize};

/// The centre of each band, an octave apart, as every ten-band equaliser has them.
pub const BANDS_HZ: [u32; 10] = [31, 62, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000];
/// How far a band goes either way. Further than this is not shaping, it is breaking.
pub const MAX_GAIN_DB: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EqualizerPreset {
    Flat,
    BassBoost,
    BassReducer,
    TrebleBoost,
    Vocal,
    Rock,
    Pop,
    Electronic,
    HipHop,
    Jazz,
    Classical,
    Acoustic,
    Quiet,
    Spoken,
    Custom,
}

impl EqualizerPreset {
    pub const ALL: [EqualizerPreset; 15] = [
        EqualizerPreset::Flat,
        EqualizerPreset::BassBoost,
        EqualizerPreset::BassReducer,
        EqualizerPreset::TrebleBoost,
        EqualizerPreset::Vocal,
        EqualizerPreset::Rock,
        EqualizerPreset::Pop,
        EqualizerPreset::Electronic,
        EqualizerPreset::HipHop,
        EqualizerPreset::Jazz,
        EqualizerPreset::Classical,
        EqualizerPreset::Acoustic,
        EqualizerPreset::Quiet,
        EqualizerPreset::Spoken,
        EqualizerPreset::Custom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EqualizerPreset::Flat => "Flat",
            EqualizerPreset::BassBoost => "Bass boost",
            EqualizerPreset::BassReducer => "Less bass",
            EqualizerPreset::TrebleBoost => "Treble boost",
            EqualizerPreset::Vocal => "Vocals",
            EqualizerPreset::Rock => "Rock",
            EqualizerPreset::Pop => "Pop",
            EqualizerPreset::Electronic => "Electronic",
            EqualizerPreset::HipHop => "Hip-hop",
            EqualizerPreset::Jazz => "Jazz",
            EqualizerPreset::Classical => "Classical",
            EqualizerPreset::Acoustic => "Acoustic",
            EqualizerPreset::Quiet => "Quiet listening",
            EqualizerPreset::Spoken => "Spoken word",
            EqualizerPreset::Custom => "Your own",
        }
    }

    /// The curve, the Kotlin presets' numbers; None for the listener's own.
    pub fn gains(self) -> Option<[f32; 10]> {
        Some(match self {
            EqualizerPreset::Flat => [0.0; 10],
            EqualizerPreset::BassBoost => [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            EqualizerPreset::BassReducer => [-6.0, -5.0, -4.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            EqualizerPreset::TrebleBoost => [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
            EqualizerPreset::Vocal => [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
            EqualizerPreset::Rock => [5.0, 4.0, 3.0, 1.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0],
            EqualizerPreset::Pop => [-1.0, 0.0, 2.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0, -1.0],
            EqualizerPreset::Electronic => [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 4.0, 5.0],
            EqualizerPreset::HipHop => [5.0, 4.0, 2.0, 3.0, -1.0, -1.0, 1.0, 0.0, 2.0, 3.0],
            EqualizerPreset::Jazz => [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0],
            EqualizerPreset::Classical => [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
            EqualizerPreset::Acoustic => [3.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0],
            EqualizerPreset::Quiet => [5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 4.0],
            EqualizerPreset::Spoken => [-3.0, -2.0, 0.0, 1.0, 3.0, 4.0, 3.0, 1.0, -1.0, -3.0],
            EqualizerPreset::Custom => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Equalizer {
    pub enabled: bool,
    pub preset: EqualizerPreset,
    /// Decibels for each band, used when the preset is Custom.
    pub custom_gains: [f32; 10],
    /// Decibels added before the bands.
    pub preamp_db: f32,
}

impl Default for Equalizer {
    fn default() -> Self {
        Equalizer { enabled: false, preset: EqualizerPreset::Flat, custom_gains: [0.0; 10], preamp_db: 0.0 }
    }
}

impl Equalizer {
    /// The curve in force, each value within range whatever was stored.
    pub fn gains(&self) -> [f32; 10] {
        let raw = self.preset.gains().unwrap_or(self.custom_gains);
        raw.map(|g| if g.is_finite() { g.clamp(-MAX_GAIN_DB, MAX_GAIN_DB) } else { 0.0 })
    }

    /// Whether it changes the sound at all; a flat or switched-off equaliser should cost nothing.
    pub fn shapes_sound(&self) -> bool {
        self.enabled && (self.gains().iter().any(|g| *g != 0.0) || self.preamp_db != 0.0)
    }
}

pub fn band_label(hz: u32) -> String {
    if hz >= 1000 { format!("{}k", hz / 1000) } else { hz.to_string() }
}

/// The volume booster, a percentage of the amplitude, as Noctorium-Base's `VolumeBoost`.
pub mod boost {
    pub const OFF: u32 = 100;
    pub const MAX: u32 = 1_000;
    pub const LOUD: u32 = 200;
    pub const STEP: u32 = 10;
    pub const LOUD_WARNING: &str = "Loud: mind your ears and your speakers";
    /// Where the limiter holds the peaks, a touch under full scale.
    pub const CEILING: f32 = 0.944;
    pub const CLIP_KNEE: f32 = 0.85;
    pub const LOOKAHEAD_MS: f32 = 5.0;
    pub const RELEASE_MS: f32 = 60.0;

    pub fn clamp(percent: u32) -> u32 {
        percent.clamp(OFF, MAX)
    }
    pub fn is_on(percent: u32) -> bool {
        clamp(percent) > OFF
    }
    pub fn is_loud(percent: u32) -> bool {
        clamp(percent) > LOUD
    }
    pub fn factor(percent: u32) -> f32 {
        clamp(percent) as f32 / 100.0
    }
    /// How hard the limiter is driven: half the boost, in decibels, so the rest is the limiter's.
    pub fn drive(percent: u32) -> f32 {
        factor(percent).sqrt()
    }
    pub fn times_label(percent: u32) -> String {
        let factor = factor(percent);
        if (factor - factor.round()).abs() < 0.005 {
            format!("×{}", factor.round() as u32)
        } else {
            format!("×{factor:.1}")
        }
    }
}

/// How what plays is shaped: what the window holds, turned into mpv's `af` chain and volume.
#[derive(Clone, Debug, PartialEq)]
pub struct Shaping {
    pub equalizer: Equalizer,
    pub boost_percent: u32,
    /// The listener's volume, 0 to 1.
    pub volume: f32,
    /// Evens out how loud songs are against each other.
    pub even_loudness: bool,
}

impl Shaping {
    /// mpv's own volume, a percentage: the listener's, or 100 while the booster carries it in its gain.
    /// mpv's volume clips at the output, and boosting it past a hundred only squares the waves off.
    pub fn mpv_volume(&self) -> f64 {
        if boost::is_on(self.boost_percent) { 100.0 } else { (self.volume.clamp(0.0, 1.0) * 100.0) as f64 }
    }

    /// The `af` chain: the equaliser, then evening out, then the booster behind its limiter and clipper.
    pub fn filters(&self) -> String {
        let mut chain: Vec<String> = Vec::new();
        if let Some(eq) = equalizer_filter(&self.equalizer) {
            chain.push(eq);
        }
        if self.even_loudness {
            // EBU R128 aiming at streaming's -14 LUFS, one pass, so it settles over the first seconds.
            chain.push("@noctorium-loudness:lavfi=[loudnorm=I=-14:TP=-1.5:LRA=11]".into());
        }
        if let Some(b) = self.boost_filter() {
            chain.push(b);
        }
        chain.join(",")
    }

    fn boost_filter(&self) -> Option<String> {
        let eq_lifts = self.equalizer.shapes_sound()
            && (self.equalizer.gains().iter().any(|g| *g > 0.0) || self.equalizer.preamp_db > 0.0);
        let on = boost::is_on(self.boost_percent);
        if !on && !eq_lifts {
            return None;
        }
        let gain = if on { boost::factor(self.boost_percent) * self.volume.clamp(0.0, 1.0) } else { 1.0 };
        let drive = if on { boost::drive(self.boost_percent) } else { 1.0 };
        let knee = boost::CLIP_KNEE * boost::CEILING;
        let room = boost::CEILING - knee;
        let over = format!("((abs(val(ch))-{knee:.6})/{room:.6})");
        let clipper = format!(
            "aeval=exprs='if(lte(abs(val(ch)),{knee:.6}),val(ch),sgn(val(ch))*({knee:.6}+{room:.6}*{over}/sqrt(1+{over}*{over})))':c=same"
        );
        Some(format!(
            "@noctorium-boost:lavfi=[volume={gain:.5},alimiter=level_in={:.4}:level_out={drive:.4}:limit={:.3}:attack={:.0}:release={:.0}:level=0,{clipper}]",
            1.0 / drive,
            boost::CEILING,
            2.0 * boost::LOOKAHEAD_MS,
            boost::RELEASE_MS,
        ))
    }
}

fn equalizer_filter(equalizer: &Equalizer) -> Option<String> {
    if !equalizer.shapes_sound() {
        return None;
    }
    let gains = equalizer.gains();
    let highest = gains.iter().cloned().fold(0.0f32, f32::max);
    // Half the highest lift taken off before the bands, so a bass boost has room rather than clipping.
    let preamp = equalizer.preamp_db - highest.max(0.0) / 2.0;
    let mut stages: Vec<String> = Vec::new();
    if preamp != 0.0 {
        stages.push(format!("volume={preamp:.1}dB"));
    }
    for (hz, gain) in BANDS_HZ.iter().zip(gains) {
        if gain != 0.0 {
            stages.push(format!("equalizer=f={hz}:t=o:w=1:g={gain:.1}"));
        }
    }
    (!stages.is_empty()).then(|| format!("@noctorium-eq:lavfi=[{}]", stages.join(",")))
}

/// The edges between the visualiser's ten bands, an octave apart around the equaliser's centres.
pub const BAND_EDGES_HZ: [u32; 9] = [44, 88, 177, 354, 707, 1_414, 2_828, 5_657, 11_314];
/// The quietest a band shows; anything lower is an empty bar.
pub const FLOOR_DB: f32 = -60.0;
pub const METER_LABEL: &str = "noctorium-meter";

/// The visualiser's measuring filter, Noctorium-Base's `spectrumMeter(2)`: the sound split in two, one half
/// played as it was, the other split into ten bands by a crossover; both measured by `astats`, which puts its
/// figures in the filter's metadata, and the bands then let go of so only the sound is heard.
pub fn spectrum_meter() -> String {
    let outputs: String = (0..BAND_EDGES_HZ.len() + 1).map(|i| format!("[b{i}]")).collect();
    let edges = BAND_EDGES_HZ.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
    let stats = "astats=metadata=1:reset=1:measure_perchannel=Peak_level+RMS_level:measure_overall=none";
    format!(
        "@{METER_LABEL}:lavfi=[aformat=channel_layouts=stereo,asplit=2[main][analysis];[analysis]pan=mono|c0=0.5*c0+0.5*c1,acrossover=split={edges}{outputs};{outputs}amerge=inputs={}[bands];[main][bands]amerge=inputs=2,{stats},pan=stereo|c0=c0|c1=c1]",
        BAND_EDGES_HZ.len() + 1
    )
}

/// What the meter measured: the ten bands, and each channel's peak and loudness, from 0 to 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    pub bands: Vec<f32>,
    pub peaks: Vec<f32>,
    pub rms: Vec<f32>,
}

fn level(decibels: f32) -> f32 {
    if decibels.is_nan() { 0.0 } else { ((decibels - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0) }
}

fn decibels_in(text: &str) -> Option<f32> {
    match text.trim().to_lowercase().as_str() {
        "-inf" | "-infinity" => Some(f32::NEG_INFINITY),
        "inf" | "+inf" | "infinity" => Some(f32::INFINITY),
        "nan" | "-nan" => Some(f32::NAN),
        other => other.parse().ok(),
    }
}

/// A frame out of the meter's `af-metadata`: `"lavfi.astats.1.RMS_level": "-26.79"`. The first two
/// channels are the sound's own, the rest its bands. `offset_db` takes back what the booster carries.
pub fn analysis_from(metadata: &serde_json::Value, offset_db: f32) -> Option<Analysis> {
    let values = metadata.as_object()?;
    let db = |channel: usize, measure: &str| {
        values.get(&format!("lavfi.astats.{channel}.{measure}")).and_then(|v| v.as_str()).and_then(decibels_in).map(|d| d + offset_db)
    };
    let channels = (1..64).take_while(|c| db(*c, "RMS_level").is_some()).count();
    if channels == 0 {
        return None;
    }
    let own = 2.min(channels);
    Some(Analysis {
        bands: (own + 1..=channels).map(|c| level(db(c, "RMS_level").unwrap_or(f32::NEG_INFINITY))).collect(),
        peaks: (1..=own).map(|c| level(db(c, "Peak_level").unwrap_or(f32::NEG_INFINITY))).collect(),
        rms: (1..=own).map(|c| level(db(c, "RMS_level").unwrap_or(f32::NEG_INFINITY))).collect(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_meter_frame_is_read_into_channels_and_bands() {
        let mut values = serde_json::Map::new();
        for c in 1..=12 {
            values.insert(format!("lavfi.astats.{c}.RMS_level"), format!("-{}", c * 4).into());
            values.insert(format!("lavfi.astats.{c}.Peak_level"), "-inf".into());
        }
        let frame = super::analysis_from(&serde_json::Value::Object(values), 0.0).unwrap();
        assert_eq!(frame.rms.len(), 2);
        assert_eq!(frame.bands.len(), 10);
        assert_eq!(frame.peaks[0], 0.0);
        assert!((frame.rms[0] - (56.0 / 60.0)).abs() < 1e-5);
        assert!(super::spectrum_meter().contains("acrossover=split=44 88"));
    }

    use super::*;

    fn shaping() -> Shaping {
        Shaping { equalizer: Equalizer::default(), boost_percent: 100, volume: 0.5, even_loudness: false }
    }

    #[test]
    fn nothing_switched_on_costs_nothing() {
        let s = shaping();
        assert_eq!(s.filters(), "");
        assert_eq!(s.mpv_volume(), 50.0);
    }

    #[test]
    fn the_equaliser_makes_room_for_its_lift() {
        let mut s = shaping();
        s.equalizer = Equalizer { enabled: true, preset: EqualizerPreset::BassBoost, ..Equalizer::default() };
        let chain = s.filters();
        assert!(chain.starts_with("@noctorium-eq:lavfi=[volume=-3.0dB,equalizer=f=31:t=o:w=1:g=6.0"), "{chain}");
        assert!(chain.contains("@noctorium-boost"), "a lift is held under the ceiling: {chain}");
    }

    #[test]
    fn the_booster_takes_the_volume_into_its_gain() {
        let mut s = shaping();
        s.boost_percent = 300;
        assert_eq!(s.mpv_volume(), 100.0);
        assert!(s.filters().contains("volume=1.50000"), "{}", s.filters());
        assert_eq!(boost::times_label(300), "×3");
        assert_eq!(boost::times_label(250), "×2.5");
    }

    #[test]
    fn a_custom_curve_is_held_in_range() {
        let eq = Equalizer { enabled: true, preset: EqualizerPreset::Custom, custom_gains: [40.0, -40.0, f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0], preamp_db: 0.0 };
        assert_eq!(&eq.gains()[..3], &[12.0, -12.0, 0.0]);
    }
}
