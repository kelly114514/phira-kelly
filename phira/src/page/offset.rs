prpr_l10n::tl_file!("cali");

use std::borrow::Cow;

use super::{Page, SharedState};
use crate::{get_data, get_data_mut, save_data};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    config::{MAX_INPUT_OFFSET_MS, MIN_INPUT_OFFSET_MS},
    core::{ParticleEmitter, ResourcePack, NOTE_WIDTH_RATIO_BASE},
    ext::{create_audio_manger, semi_black, semi_white, RectExt, SafeTexture, ScaleType},
    judge::Judge,
    time::TimeManager,
    ui::{DRectButton, Slider, Ui},
};
use sasa::{AudioClip, AudioManager, Music, MusicParams, PlaySfxParams, Sfx};

pub struct OffsetPage {
    _audio: AudioManager,
    cali: Music,
    cali_hit: Sfx,

    tm: TimeManager,
    cali_last: bool,

    click: SafeTexture,
    _hit_fx: SafeTexture,
    emitter: ParticleEmitter,
    color: Color,

    slider: Slider,
    input_slider: Slider,
    apply_btn: DRectButton,
    clear_btn: DRectButton,

    input_samples: Vec<f64>,
    dispatch_samples: Vec<f64>,

    touched: bool,
    touch: Option<(f32, f32)>,
}

impl OffsetPage {
    const FADE_TIME: f32 = 0.8;
    const MIN_INPUT_SAMPLES: usize = 30;
    const MAX_INPUT_SAMPLES: usize = 60;
    const MAX_SAMPLE_ERROR: f64 = 0.35;
    const STABLE_MAD: f64 = 0.020;

    pub async fn new() -> Result<Self> {
        let mut audio = create_audio_manger(&get_data().config)?;
        let cali = audio.create_music(
            AudioClip::new(load_file("cali.ogg").await?)?,
            MusicParams {
                amplifier: get_data().config.volume_music,
                loop_mix_time: 0.,
                ..Default::default()
            },
        )?;
        let cali_hit = audio.create_sfx(AudioClip::new(load_file("cali_hit.ogg").await?)?, None)?;

        let mut tm = TimeManager::new(1., true);
        tm.force = 3e-2;

        let respack = ResourcePack::from_path(get_data().config.res_pack_path.as_ref())
            .await
            .context("Failed to load resource pack")?;
        let click = respack.note_style.click.clone();
        let emitter = ParticleEmitter::new(&respack, get_data().config.note_scale, respack.info.hide_particles)?;
        Ok(Self {
            _audio: audio,
            cali,
            cali_hit,

            tm,
            cali_last: false,

            click,
            _hit_fx: respack.hit_fx,
            emitter,
            color: respack.info.fx_perfect(),

            slider: Slider::new(-500.0..500.0, 5.),
            input_slider: Slider::new(MIN_INPUT_OFFSET_MS as f32..MAX_INPUT_OFFSET_MS as f32, 1.),
            apply_btn: DRectButton::new(),
            clear_btn: DRectButton::new(),

            input_samples: Vec::new(),
            dispatch_samples: Vec::new(),

            touched: false,
            touch: None,
        })
    }

    fn sample_stats(&self) -> Option<(f64, f64)> {
        calibration_stats(&self.input_samples)
    }

    fn record_input_sample(&mut self, touch: &Touch, audio_offset: f32) {
        let Some(age) = Judge::touch_event_age(touch) else {
            return;
        };
        self.dispatch_samples.push(age);
        if self.dispatch_samples.len() > Self::MAX_INPUT_SAMPLES {
            self.dispatch_samples.remove(0);
        }

        // The existing audio/chart offset remains part of the expected beat.
        // Input compensation is deliberately not applied here: calibration
        // estimates the total residual correction from raw timestamped input.
        let event_time = self.tm.now() - age;
        let adjusted = event_time - audio_offset as f64;
        let mut error = (adjusted - 1.).rem_euclid(2.);
        if error > 1. {
            error -= 2.;
        }
        if error.abs() <= Self::MAX_SAMPLE_ERROR {
            self.input_samples.push(error);
            if self.input_samples.len() > Self::MAX_INPUT_SAMPLES {
                self.input_samples.remove(0);
            }
        }
    }
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.
    } else {
        values[mid]
    })
}

fn calibration_stats(samples: &[f64]) -> Option<(f64, f64)> {
    let center = median(samples)?;
    let deviations: Vec<_> = samples.iter().map(|value| (value - center).abs()).collect();
    Some((center, median(&deviations).unwrap_or_default()))
}

impl Page for OffsetPage {
    fn can_play_bgm(&self) -> bool {
        false
    }

    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn exit(&mut self) -> Result<()> {
        save_data()?;
        Ok(())
    }

    fn enter(&mut self, _s: &mut SharedState) -> Result<()> {
        self.cali.seek_to(0.)?;
        self.cali.play()?;
        self.tm.reset();
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        save_data()?;
        self.tm.pause();
        self.cali.pause()?;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.tm.resume();
        self.cali.play()?;
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        let config = &mut get_data_mut().config;
        let mut offset = config.offset * 1000.;
        if self.slider.touch(touch, t, &mut offset).is_some() {
            config.offset = offset / 1000.;
            return Ok(true);
        }
        let mut input_offset = config.input_offset_ms as f32;
        if self.input_slider.touch(touch, t, &mut input_offset).is_some() {
            config.input_offset_ms = input_offset.round().clamp(MIN_INPUT_OFFSET_MS as f32, MAX_INPUT_OFFSET_MS as f32) as i16;
            return Ok(true);
        }
        if self.clear_btn.touch(touch, t) {
            self.input_samples.clear();
            self.dispatch_samples.clear();
            return Ok(true);
        }
        if self.apply_btn.touch(touch, t) {
            if self.input_samples.len() >= Self::MIN_INPUT_SAMPLES {
                if let Some((center, mad)) = self.sample_stats() {
                    if mad <= Self::STABLE_MAD {
                        config.input_offset_ms = (center * 1000.).round().clamp(MIN_INPUT_OFFSET_MS as f64, MAX_INPUT_OFFSET_MS as f64) as i16;
                    }
                }
            }
            return Ok(true);
        }
        if touch.phase == TouchPhase::Started && touch.position.x < 0. {
            self.touched = true;
            self.record_input_sample(touch, config.offset);
        }
        Ok(false)
    }

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        if !self.cali.paused() {
            let pos = self.cali.position();
            let now = self.tm.now();
            if now > 2. {
                self.tm.seek_to(now - 2.);
                self.tm.dont_wait();
            }
            let now = self.tm.now();
            if now - pos >= -1. {
                self.tm.update(pos);
            }
        }
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        s.render_fader(ui, |ui| {
            let lf = -0.92;
            let mut r = ui.content_rect();
            r.w += r.x - lf;
            r.x = lf;
            ui.fill_path(&r.rounded(0.02), semi_black(0.4));

            let ct = (-0.4, r.bottom() - 0.12);
            let hw = 0.4;
            let hh = 0.005;
            ui.fill_rect(Rect::new(ct.0 - hw, ct.1 - hh, hw * 2., hh * 2.), WHITE);

            let ot = t;

            let config = &get_data().config;
            let mut t = self.tm.now() as f32 - config.offset;
            if t < 0. {
                t += 2.;
            }
            if t >= 2. {
                t -= 2.;
            }
            let ny = ct.1 + (t - 1.) * 0.6;
            if self.touched {
                self.touch = Some((ot, ny));
                self.touched = false;
            }
            if t <= 1. {
                let w = NOTE_WIDTH_RATIO_BASE as f32 * config.note_scale * 2.;
                let h = w * self.click.height() / self.click.width();
                let r = Rect::new(ct.0 - w / 2., ny, w, h);
                ui.fill_rect(r, (*self.click, r, ScaleType::Fit));
                self.cali_last = true;
            } else {
                if self.cali_last {
                    let g = ui.to_global(ct);
                    self.emitter.emit_at(vec2(g.0, g.1), 0., self.color);
                    let _ = self.cali_hit.play(PlaySfxParams {
                        amplifier: config.volume_sfx,
                    });
                }
                self.cali_last = false;
            }

            if let Some((time, pos)) = &self.touch {
                let p = (ot - time) / Self::FADE_TIME;
                if p > 1. {
                    self.touch = None;
                } else {
                    let p = p.max(0.);
                    let c = Color {
                        a: (if p <= 0.5 { 1. } else { (1. - p) * 2. }) * self.color.a,
                        ..self.color
                    };
                    ui.fill_rect(Rect::new(ct.0 - hw, pos - hh, hw * 2., hh * 2.), c);
                }
            }

            let offset = config.offset * 1000.;
            ui.text(tl!("audio-offset")).pos(0.46, -0.22).size(0.42).color(semi_white(0.8)).draw();
            self.slider
                .render(ui, Rect::new(0.46, -0.16, 0.45, 0.16), ot, offset, format!("{offset:.0}ms"));

            ui.text(tl!("input-offset")).pos(0.46, 0.08).size(0.42).color(semi_white(0.8)).draw();
            self.input_slider.render(
                ui,
                Rect::new(0.46, 0.14, 0.45, 0.16),
                ot,
                config.input_offset_ms as f32,
                format!("{:+}ms", config.input_offset_ms),
            );

            let count = self.input_samples.len();
            let dispatch_ms = median(&self.dispatch_samples).unwrap_or_default() * 1000.;
            let (summary, ready) = if let Some((center, mad)) = self.sample_stats() {
                let stable = count >= Self::MIN_INPUT_SAMPLES && mad <= Self::STABLE_MAD;
                (
                    tl!(
                        "input-stats",
                        "count" => count,
                        "target" => Self::MIN_INPUT_SAMPLES,
                        "median" => (center * 1000.).round() as i32,
                        "mad" => (mad * 1000.).round() as i32,
                        "dispatch" => dispatch_ms.round() as i32
                    )
                    .to_string(),
                    stable,
                )
            } else {
                (tl!("input-stats-empty", "target" => Self::MIN_INPUT_SAMPLES).to_string(), false)
            };
            ui.text(summary)
                .pos(0.46, 0.37)
                .size(0.34)
                .multiline()
                .max_width(0.45)
                .color(semi_white(0.78))
                .draw();
            ui.text(if ready { tl!("input-stable") } else { tl!("input-tap-hint") })
                .pos(0.46, 0.49)
                .size(0.32)
                .multiline()
                .max_width(0.45)
                .color(if ready { Color::from_hex_rgb(0x81c784) } else { semi_white(0.58) })
                .draw();

            let apply = Rect::new(0.46, 0.64, 0.27, 0.1);
            let clear = Rect::new(0.75, 0.64, 0.16, 0.1);
            if ready {
                self.apply_btn.render_text(ui, apply, ot, tl!("input-apply"), 0.4, true);
            } else {
                self.apply_btn
                    .render_text_color(ui, apply, ot, tl!("input-apply"), 0.4, false, semi_white(0.35));
            }
            self.clear_btn.render_text(ui, clear, ot, tl!("input-clear"), 0.4, false);
        });

        self.emitter.draw(get_frame_time());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_uses_robust_median_and_mad() {
        let samples = [0.020, 0.021, 0.019, 0.022, 0.300];
        let (center, mad) = calibration_stats(&samples).unwrap();
        assert!((center - 0.021).abs() < 1e-9);
        assert!((mad - 0.001).abs() < 1e-9);
    }
}
