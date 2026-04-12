use egui_knob::{Knob, KnobStyle, LabelPosition};
use nih_plug::prelude::*;
use nih_plug_egui::{
    EguiState, create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
};
use std::sync::Arc;

use crate::{
    envelope::{Envelope, EnvelopeParams},
    oscillator::Oscillator,
    voice::Voice,
};

mod envelope;
mod oscillator;
mod voice;

const MAX_VOICES: usize = 16;

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum WaveType {
    Sine,
    Square,
    Saw,
    Triangle,
}

impl WaveType {
    pub fn next(&self) -> Self {
        match self {
            Self::Sine => Self::Square,
            Self::Square => Self::Saw,
            Self::Saw => Self::Triangle,
            Self::Triangle => Self::Sine,
        }
    }
}

pub struct Serenity {
    params: Arc<SerenityParams>,
    voices: Vec<Voice>,
    envelope: Envelope,
    pitch_bend: f32,
    sample_rate: f32, // I'm not sure I really need this at the vst level, only really at the oscillators, but I'll hold onto it for the future rn.
}

impl Default for Serenity {
    fn default() -> Self {
        Serenity {
            params: Arc::new(SerenityParams::default()),
            voices: (0..MAX_VOICES).map(|_| Voice::default()).collect(), // If I have a specific amount maybe I don't want a vector, maybe just an array?
            envelope: Envelope::default(),
            pitch_bend: 0.5,
            sample_rate: 44100.0,
        }
    }
}

impl Serenity {
    fn calculate_wave(&mut self) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;

        for voice in self.voices.iter_mut() {
            if voice.midi_note_id.is_none() {
                continue;
            }
            let result = voice.calculate_wave(
                self.params.wave_type.value(),
                self.params.detune.value(),
                self.pitch_bend,
                self.params.cutoff.value(),
            );
            left += result.0;
            right += result.1;
        }

        let active = self
            .voices
            .iter()
            .filter(|v| v.midi_note_id.is_some())
            .count()
            .max(1);
        (left / active as f32, right / active as f32) // Could result in it being too quiet I think, maybe there is some trick?
    }

    fn find_new_voice(&mut self) -> usize {
        let mut slot = 0;
        let mut age = 0;
        let mut empty: Option<usize> = None;
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if voice.age > age {
                age = voice.age;
                slot = i;
            }
            if voice.midi_note_id.is_some() {
                voice.age += 1;
            } else if empty.is_none() {
                empty = Some(i);
            }
        }
        empty.unwrap_or(slot)
    }

    fn update_oscillator_count(&mut self, count: usize) {
        let oscillator_count = self.voices[0].oscillators.len(); // I might have to fix this, now that I use voices!

        for voice in self.voices.iter_mut() {
            if count > oscillator_count {
                for _ in 0..(count - oscillator_count) {
                    voice.oscillators.push(Oscillator::default());
                }
            } else if count < oscillator_count {
                voice.oscillators.truncate(count);
            }
        }
    }
}

#[derive(Params)]
struct SerenityParams {
    #[id = "usemidi"]
    pub use_midi: BoolParam,
    #[id = "wavetype"]
    pub wave_type: EnumParam<WaveType>,
    #[id = "oscillators"]
    pub oscillators: IntParam,
    #[id = "detune"]
    pub detune: FloatParam,
    #[id = "lowpass"]
    pub cutoff: FloatParam,
    #[nested(group = "ADSR")]
    pub envelope: EnvelopeParams,
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for SerenityParams {
    fn default() -> Self {
        SerenityParams {
            use_midi: BoolParam::new("USE MIDI", false),
            wave_type: EnumParam::new("Wave Type", WaveType::Sine),
            oscillators: IntParam::new("Oscillators", 1, IntRange::Linear { min: 1, max: 5 }), // I honestly don't know what max should be, I chose 5 because I'm pretty sure serum uses 5 but I should look into why?
            detune: FloatParam::new(
                "Detune",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 50.0,
                },
            ),
            cutoff: FloatParam::new("Cutoff", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            envelope: EnvelopeParams::default(),
            editor_state: EguiState::from_size(400, 400),
        }
    }
}

impl Vst3Plugin for Serenity {
    const VST3_CLASS_ID: [u8; 16] = *b"SerenityPlugXXXX";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

impl ClapPlugin for Serenity {
    const CLAP_ID: &'static str = "dev.stpierre.serenity";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A simple synth");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer];
}

impl Plugin for Serenity {
    const NAME: &'static str = "Serenity";
    const VENDOR: &'static str = "Gabriel St Pierre";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = "0.1.0";

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.envelope.set_sample_rate(buffer_config.sample_rate);

        true
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let egui_state = params.editor_state.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                ResizableWindow::new("res-wind")
                    .min_size(Vec2::new(400.0, 400.0))
                    .show(egui_ctx, egui_state.as_ref(), |ui| {
                        if ui.add(egui::Button::new("Change Waveform")).clicked() {
                            setter
                                .set_parameter(&params.wave_type, params.wave_type.value().next());
                        }

                        ui.horizontal(|ui| {
                            let mut attack_value = params.envelope.attack.value();
                            if ui
                                .add(
                                    Knob::new(&mut attack_value, 0.0, 10000.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Attack", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.envelope.attack, attack_value);
                            }

                            let mut decay_value = params.envelope.decay.value();
                            if ui
                                .add(
                                    Knob::new(&mut decay_value, 0.0, 10000.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Decay", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.envelope.decay, decay_value);
                            }

                            let mut sustain_value = params.envelope.sustain.value();
                            if ui
                                .add(
                                    Knob::new(&mut sustain_value, 0.0, 1.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Sustain", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.envelope.sustain, sustain_value);
                            }

                            let mut release_value = params.envelope.release.value();
                            if ui
                                .add(
                                    Knob::new(&mut release_value, 0.0, 10000.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Release", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.envelope.release, release_value);
                            }

                            ui.allocate_space(egui::Vec2::splat(2.0));
                        });

                        ui.horizontal(|ui| {
                            // Oscillator knob is a bit (or a lot) sensitive.

                            let mut oscillator_count = params.oscillators.value() as f32;
                            if ui
                                .add(
                                    Knob::new(&mut oscillator_count, 0.0, 5.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_step(1.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Oscillators", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.oscillators, oscillator_count as i32);
                            }

                            let mut detune_value = params.detune.value();
                            if ui
                                .add(
                                    Knob::new(&mut detune_value, 0.0, 50.0, KnobStyle::Wiper)
                                        .with_size(50.0)
                                        .with_colors(
                                            egui::Color32::GRAY,
                                            egui::Color32::WHITE,
                                            egui::Color32::WHITE,
                                        )
                                        .with_label("Detune", LabelPosition::Top),
                                )
                                .changed()
                            {
                                setter.set_parameter(&params.detune, detune_value);
                            }
                        });
                    });
            },
        )
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        for voice in &mut self.voices {
            voice.envelope.update_params(&self.params.envelope);
            if voice.envelope.is_idle() && voice.midi_note_id.is_some() {
                voice.voice_off();
            }
        }

        self.envelope.update_params(&self.params.envelope); // Should be called every 64 - 128 samples rather than ever single sample -  https://nih-plug.robbertvanderhelm.nl/nih_plug/buffer/struct.Buffer.html

        let desired_oscillator_count = self.params.oscillators.value() as usize;
        self.update_oscillator_count(desired_oscillator_count);

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            let (left, right) = if self.params.use_midi.value() {
                while let Some(event) = next_event {
                    if event.timing() > sample_id as u32 {
                        break;
                    }

                    match event {
                        NoteEvent::NoteOn { note, .. } => {
                            let slot = self.find_new_voice();
                            self.voices.get_mut(slot).unwrap().set_voice(note);
                        }
                        NoteEvent::NoteOff { note, .. } => {
                            for voice in self.voices.iter_mut() {
                                if voice.midi_note_id.is_some_and(|id| id == note) {
                                    voice.envelope.note_off();
                                }
                            }
                        }
                        NoteEvent::MidiPitchBend { value, .. } => {
                            self.pitch_bend = value;
                        }
                        _ => (),
                    }

                    next_event = context.next_event();
                }

                self.calculate_wave()
            } else {
                (0.0, 0.0)
            };

            for (channel_idx, sample) in channel_samples.into_iter().enumerate() {
                *sample = match channel_idx {
                    0 => left,
                    1 => right,
                    _ => 0.0,
                }
            }
        }

        ProcessStatus::Normal
    }
}

nih_export_vst3!(Serenity);
nih_export_clap!(Serenity);
