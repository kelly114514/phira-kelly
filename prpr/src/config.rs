//! Configuration module of the playing environment.\
//! e.g. player name, volume, speed, autoplay, etc.

use bitflags::bitflags;
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer, Serialize};

pub const DEFAULT_PERFECT_MS: u16 = 80;
pub const DEFAULT_GOOD_MS: u16 = 160;
pub const DEFAULT_BAD_MS: u16 = 220;
pub const MAX_JUDGEMENT_MS: u16 = 500;
pub const MIN_INPUT_OFFSET_MS: i16 = -100;
pub const MAX_INPUT_OFFSET_MS: i16 = 100;

fn deserialize_input_offset_ms<'de, D>(deserializer: D) -> Result<i16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = i64::deserialize(deserializer)?;
    Ok(value.clamp(MIN_INPUT_OFFSET_MS as i64, MAX_INPUT_OFFSET_MS as i64) as i16)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgementWindows {
    pub perfect_ms: u16,
    pub good_ms: u16,
    pub bad_ms: u16,
}

impl<'de> Deserialize<'de> for JudgementWindows {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(default, rename_all = "camelCase")]
        struct RawWindows {
            perfect_ms: i64,
            good_ms: i64,
            bad_ms: i64,
        }

        impl Default for RawWindows {
            fn default() -> Self {
                Self {
                    perfect_ms: DEFAULT_PERFECT_MS as i64,
                    good_ms: DEFAULT_GOOD_MS as i64,
                    bad_ms: DEFAULT_BAD_MS as i64,
                }
            }
        }

        let raw = RawWindows::deserialize(deserializer)?;
        let clamp = |value: i64| value.clamp(0, MAX_JUDGEMENT_MS as i64) as u16;
        let mut windows = Self {
            perfect_ms: clamp(raw.perfect_ms),
            good_ms: clamp(raw.good_ms),
            bad_ms: clamp(raw.bad_ms),
        };
        windows.normalize();
        Ok(windows)
    }
}

impl Default for JudgementWindows {
    fn default() -> Self {
        Self {
            perfect_ms: DEFAULT_PERFECT_MS,
            good_ms: DEFAULT_GOOD_MS,
            bad_ms: DEFAULT_BAD_MS,
        }
    }
}

impl JudgementWindows {
    pub fn normalize(&mut self) {
        self.perfect_ms = self.perfect_ms.min(MAX_JUDGEMENT_MS);
        self.good_ms = self.good_ms.min(MAX_JUDGEMENT_MS).max(self.perfect_ms);
        self.bad_ms = self.bad_ms.min(MAX_JUDGEMENT_MS).max(self.good_ms);
    }

    #[inline]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[inline]
    pub fn perfect(&self) -> f64 {
        self.perfect_ms as f64 / 1000.
    }

    #[inline]
    pub fn good(&self) -> f64 {
        self.good_ms as f64 / 1000.
    }

    #[inline]
    pub fn bad(&self) -> f64 {
        self.bad_ms as f64 / 1000.
    }
}

pub static TIPS: Lazy<Vec<String>> = Lazy::new(|| include_str!("tips.txt").split('\n').map(str::to_owned).collect());

bitflags! {
    #[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(transparent)]
    pub struct Mods: i32 {
        const AUTOPLAY = 0x0001;
        const FLIP_X = 0x0002;
        const FADE_OUT = 0x0004;
        const FADE_IN = 0x0008;
        const NIGHTCORE = 0x0010;
        const RAINBOW = 0x0020;
        const NO_SHADER = 0x0040;
        const INSTANT_DEATH_AP = 0x0080;
        const INSTANT_DEATH_FC = 0x0100;

        const UNRATED = Self::AUTOPLAY.bits() | Self::NO_SHADER.bits();
    }
}

impl Mods {
    pub fn toggle_mod(&mut self, flag: Mods) {
        if self.contains(flag) {
            self.remove(flag);
        } else {
            for &conflict in Mods::conflicts(flag) {
                self.remove(conflict);
            }
            self.insert(flag);
        }
    }
    fn conflicts(flag: Mods) -> &'static [Mods] {
        match flag {
            Mods::FADE_IN => &[Mods::FADE_OUT],
            Mods::FADE_OUT => &[Mods::FADE_IN],
            Mods::INSTANT_DEATH_AP => &[Mods::INSTANT_DEATH_FC],
            Mods::INSTANT_DEATH_FC => &[Mods::INSTANT_DEATH_AP],
            _ => &[],
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "adjust_time_new")]
    pub adjust_time: bool,
    pub aggressive: bool,
    pub ap_fc_indicator: bool,
    pub aspect_ratio: Option<f32>,
    pub audio_buffer_size: Option<u32>,
    pub chart_debug: bool,
    pub disable_effect: bool,
    pub double_click_to_pause: bool,
    pub double_hint: bool,
    pub fullscreen_mode: bool,
    pub fxaa: bool,
    pub interactive: bool,
    /// Signed residual input compensation in milliseconds. Positive values
    /// move timestamped touch input earlier without changing chart/audio time.
    #[serde(deserialize_with = "deserialize_input_offset_ms")]
    pub input_offset_ms: i16,
    pub judgement_windows: JudgementWindows,
    pub judgement_window_notice_pending: bool,
    pub mods: Mods,
    pub mp_address: String,
    pub mp_enabled: bool,
    pub note_scale: f32,
    pub offline_mode: bool,
    pub offset: f32,
    pub particle: bool,
    pub player_name: String,
    pub player_rks: f32,
    pub preferred_sample_rate: Option<u32>,
    pub res_pack_path: Option<String>,
    pub sample_count: u32,
    pub show_acc: bool,
    pub show_avg_fps: bool,
    pub speed: f32,
    pub touch_debug: bool,
    pub use_keyboard: bool,
    pub volume_bgm: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,

    // for compatibility
    autoplay: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: false,
            aggressive: true,
            ap_fc_indicator: true,
            aspect_ratio: None,
            audio_buffer_size: None,
            chart_debug: false,
            disable_effect: false,
            double_click_to_pause: true,
            double_hint: true,
            fxaa: false,
            interactive: true,
            input_offset_ms: 0,
            judgement_windows: JudgementWindows::default(),
            judgement_window_notice_pending: false,
            mods: Mods::default(),
            mp_address: "mp2.phira.cn:12345".to_owned(),
            mp_enabled: false,
            note_scale: 1.0,
            offline_mode: false,
            fullscreen_mode: false,
            offset: 0.,
            particle: true,
            player_name: "Mivik".to_string(),
            player_rks: 15.,
            preferred_sample_rate: None,
            res_pack_path: None,
            sample_count: 1,
            show_acc: false,
            show_avg_fps: false,
            speed: 1.,
            touch_debug: false,
            use_keyboard: false,
            volume_music: 1.,
            volume_sfx: 1.,
            volume_bgm: 1.,

            autoplay: None,
        }
    }
}

impl Config {
    pub fn init(&mut self) {
        self.input_offset_ms = self.input_offset_ms.clamp(MIN_INPUT_OFFSET_MS, MAX_INPUT_OFFSET_MS);
        self.judgement_windows.normalize();
        if self.judgement_windows.is_default() {
            self.judgement_window_notice_pending = false;
        }
        if let Some(flag) = self.autoplay {
            self.mods.set(Mods::AUTOPLAY, flag);
        }
        #[cfg(target_env = "ohos")]
        {
            // Due to the fucking poor performance of the Maloon GPU, the sample count must be set to 1.
            self.sample_count = 1;
        }
    }

    #[inline]
    pub fn has_mod(&self, m: Mods) -> bool {
        self.mods.contains(m)
    }

    #[inline]
    pub fn autoplay(&self) -> bool {
        self.has_mod(Mods::AUTOPLAY)
    }

    #[inline]
    pub fn flip_x(&self) -> bool {
        self.has_mod(Mods::FLIP_X)
    }

    #[inline]
    pub fn input_offset(&self) -> f64 {
        self.input_offset_ms as f64 / 1000.
    }

    pub fn score_upload_allowed(&self, mods: Mods, can_rated: bool, force_original_judgement: bool) -> bool {
        !self.offline_mode
            && can_rated
            && !mods.intersects(Mods::UNRATED)
            && !self.use_keyboard
            && self.speed >= 1.0 - 1e-3
            && (force_original_judgement || self.judgement_windows.is_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgement_windows_normalize_invalid_order() {
        let mut windows = JudgementWindows {
            perfect_ms: 600,
            good_ms: 20,
            bad_ms: 10,
        };
        windows.normalize();
        assert_eq!(windows.perfect_ms, 500);
        assert_eq!(windows.good_ms, 500);
        assert_eq!(windows.bad_ms, 500);
    }

    #[test]
    fn judgement_windows_deserialize_clamps_and_repairs() {
        let windows: JudgementWindows = serde_json::from_str(r#"{"perfectMs":-15,"goodMs":700,"badMs":12}"#).unwrap();
        assert_eq!(
            windows,
            JudgementWindows {
                perfect_ms: 0,
                good_ms: 500,
                bad_ms: 500,
            }
        );
    }

    #[test]
    fn old_config_uses_default_judgement_windows() {
        let mut config: Config = serde_json::from_str("{}").unwrap();
        config.init();
        assert_eq!(config.input_offset_ms, 0);
        assert!(config.judgement_windows.is_default());
        assert!(!config.judgement_window_notice_pending);
    }

    #[test]
    fn input_offset_is_signed_and_clamped() {
        let mut positive: Config = serde_json::from_str(r#"{"inputOffsetMs":100000}"#).unwrap();
        positive.init();
        assert_eq!(positive.input_offset_ms, MAX_INPUT_OFFSET_MS);

        let mut negative: Config = serde_json::from_str(r#"{"inputOffsetMs":-100000}"#).unwrap();
        negative.init();
        assert_eq!(negative.input_offset_ms, MIN_INPUT_OFFSET_MS);
    }

    #[test]
    fn custom_windows_block_upload_without_affecting_other_guards() {
        let mut config = Config::default();
        assert!(config.score_upload_allowed(Mods::default(), true, false));
        config.input_offset_ms = 35;
        assert!(config.score_upload_allowed(Mods::default(), true, false));
        config.judgement_windows.bad_ms = 225;
        assert!(!config.score_upload_allowed(Mods::default(), true, false));
        assert!(config.score_upload_allowed(Mods::default(), true, true));
        config.offline_mode = true;
        assert!(!config.score_upload_allowed(Mods::default(), true, true));
    }

    #[test]
    fn upload_eligibility_matrix_keeps_all_existing_guards() {
        let mut config = Config::default();
        assert!(!config.score_upload_allowed(Mods::default(), false, false));
        assert!(!config.score_upload_allowed(Mods::AUTOPLAY, true, false));
        config.use_keyboard = true;
        assert!(!config.score_upload_allowed(Mods::default(), true, false));
        config.use_keyboard = false;
        config.speed = 0.99;
        assert!(!config.score_upload_allowed(Mods::default(), true, false));
    }

    #[test]
    fn notice_state_round_trips_and_reset_clears_it() {
        let mut config = Config::default();
        config.judgement_windows.bad_ms = 300;
        config.judgement_window_notice_pending = true;
        let json = serde_json::to_string(&config).unwrap();
        let mut loaded: Config = serde_json::from_str(&json).unwrap();
        loaded.init();
        assert!(loaded.judgement_window_notice_pending);

        loaded.judgement_windows = JudgementWindows::default();
        loaded.init();
        assert!(!loaded.judgement_window_notice_pending);
    }
}
