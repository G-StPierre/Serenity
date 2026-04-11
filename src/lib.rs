use nih_plug::prelude::*;
use std::sync::Arc;

use crate::{
    envelope::{Envelope, EnvelopeParams},
    oscillator::Oscillator,
};

mod envelope;
mod oscillator;

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum WaveType {
    Sine,
    Square,
    Saw,
    Triangle,
}

pub struct Serenity {
    params: Arc<SerenityParams>,
    envelope: Envelope,
    oscillators: Vec<Oscillator>,
    midi_note_id: u8,
    current_freq: f32,
    sample_rate: f32, // I'm not sure I really need this at the vst level, only really at the oscillators, but I'll hold onto it for the future rn.
}

impl Default for Serenity {
    fn default() -> Self {
        Serenity {
            params: Arc::new(SerenityParams::default()),
            envelope: Envelope::default(),
            oscillators: vec![Oscillator::default()],
            midi_note_id: 0,
            current_freq: 0.0,
            sample_rate: 44100.0,
        }
    }
}

impl Serenity {
    fn calculate_wave(&mut self, frequency: f32) -> (f32, f32) {
        // let mut sample = 0.0;

        let mut left = 0.0;
        let mut right = 0.0;

        let wave_type = self.params.wave_type.value();
        let oscillator_count = self.oscillators.len();
        let spread = self.params.detune.value();

        for (i, oscillator) in self.oscillators.iter_mut().enumerate() {
            let (offset_cents, pan) = if oscillator_count == 1 {
                (0.0, 0.0)
            } else {
                let temp = i as f32 / (oscillator_count - 1) as f32;
                (temp * (spread * 2.0) - spread, temp * 2.0 - 1.0)
            };

            let detuned_freq = frequency * 2.0_f32.powf(offset_cents / 1200.0);
            let sample = oscillator.calculate_wave(wave_type, detuned_freq);

            // Constant panning law - https://www.cs.cmu.edu/~music/icm-online/readings/panlaws/
            left += sample * ((1.0 - pan) / 2.0).sqrt() * 2.0_f32.sqrt();
            right += sample * ((1.0 - pan) / 2.0).sqrt() * 2.0_f32.sqrt();
        }

        (left, right)
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

        self.envelope.update_params(&self.params.envelope); // Should be called every 64 - 128 samples rather than ever single sample -  https://nih-plug.robbertvanderhelm.nl/nih_plug/buffer/struct.Buffer.html

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
                            self.midi_note_id = note;
                            self.current_freq = util::midi_note_to_freq(note);
                            self.envelope.note_on();
                        }
                        NoteEvent::NoteOff { .. } => {
                            self.envelope.note_off();
                        }
                        _ => (),
                    }

                    next_event = context.next_event();
                }

                self.calculate_wave(self.current_freq)
            } else {
                (0.0, 0.0)
            };

            let volume = self.envelope.next_amp();

            for (channel_idx, sample) in channel_samples.into_iter().enumerate() {
                *sample = match channel_idx {
                    0 => left * volume,
                    1 => right * volume,
                    _ => 0.0,
                }
            }
        }

        ProcessStatus::Normal
    }
}

nih_export_vst3!(Serenity);
nih_export_clap!(Serenity);
