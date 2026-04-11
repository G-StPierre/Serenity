use nih_plug::prelude::*;
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

pub struct Serenity {
    params: Arc<SerenityParams>,
    voices: Vec<Voice>,
    envelope: Envelope,
    oscillators: Vec<Oscillator>,
    sample_rate: f32, // I'm not sure I really need this at the vst level, only really at the oscillators, but I'll hold onto it for the future rn.
}

impl Default for Serenity {
    fn default() -> Self {
        Serenity {
            params: Arc::new(SerenityParams::default()),
            voices: (0..MAX_VOICES).map(|_| Voice::default()).collect(), // If I have a specific amount maybe I don't want a vector, maybe just an array?
            envelope: Envelope::default(),
            oscillators: vec![Oscillator::default()],
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
            let result =
                voice.calculate_wave(self.params.wave_type.value(), self.params.detune.value());
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
    #[nested(group = "ADSR")]
    pub envelope: EnvelopeParams,
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
            envelope: EnvelopeParams::default(),
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

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;

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

        // let voice_count = self.voices.len();

        let desired_oscillator_count = self.params.oscillators.value() as usize;
        let oscillator_count = self.oscillators.len();

        if desired_oscillator_count > oscillator_count {
            for _ in 0..(desired_oscillator_count - oscillator_count) {
                self.oscillators.push(Oscillator::default());
            }
        } else if desired_oscillator_count < oscillator_count {
            self.oscillators.truncate(desired_oscillator_count);
        }

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
                            nih_log!("assigning note {} to slot {}", note, slot);
                        }
                        NoteEvent::NoteOff { note, .. } => {
                            for voice in self.voices.iter_mut() {
                                if voice.midi_note_id.is_some_and(|id| id == note) {
                                    voice.envelope.note_off();
                                }
                            }
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
