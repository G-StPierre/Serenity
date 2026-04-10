use nih_plug::prelude::*;
use std::f32::consts;
use std::sync::Arc;

use crate::envelope::{Envelope, EnvelopeParams};

mod envelope;

#[derive(Enum, PartialEq)]
pub enum WaveType {
    Sine,
    Square,
    Saw,
    Triangle,
}

pub struct Serenity {
    params: Arc<SerenityParams>,
    envelope: Envelope,
    midi_note_id: u8,
    midi_note_freq: f32,
    sample_rate: f32,
    phase: f32,
}

impl Default for Serenity {
    fn default() -> Self {
        Serenity {
            params: Arc::new(SerenityParams::default()),
            envelope: Envelope::default(),
            midi_note_id: 0,
            midi_note_freq: 1.0,
            sample_rate: 1.0,
            phase: 0.0,
        }
    }
}

impl Serenity {
    fn calculate_wave(&mut self, frequency: f32) -> f32 {
        match self.params.wave_type.value() {
            WaveType::Sine => self.calculate_sine(frequency),
            WaveType::Square => self.calculate_square(frequency),
            WaveType::Saw => self.calculate_saw(frequency),
            WaveType::Triangle => self.calculate_triangle(frequency),
        }
    }

    fn calculate_sine(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;
        let sine = (self.phase * consts::TAU).sin();

        self.phase += phase_delta;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sine
    }

    // Seems like either Band-Limited Impulse Train or Band-limited Step Function will round this a bit and make it sound less harsh https://www.metafunction.co.uk/post/all-about-digital-oscillators-part-2-blits-bleps
    fn calculate_square(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;
        let square = if self.phase < 0.5 { 1.0 } else { -1.0 };

        self.phase += phase_delta;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        square
    }

    fn calculate_saw(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;

        let saw = (self.phase * 2.0) - 1.0;

        self.phase += phase_delta;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        saw
    }

    fn calculate_triangle(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;

        let triangle = 4.0 * (self.phase - 0.5).abs() - 1.0;

        self.phase += phase_delta;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        triangle
    }
}

#[derive(Params)]
struct SerenityParams {
    #[id = "usemidi"]
    pub use_midi: BoolParam,
    #[id = "wavetype"]
    pub wave_type: EnumParam<WaveType>,
    #[nested(group = "ADSR")]
    pub envelope: EnvelopeParams,
}

impl Default for SerenityParams {
    fn default() -> Self {
        SerenityParams {
            use_midi: BoolParam::new("USE MIDI", false),
            wave_type: EnumParam::new("Wave Type", WaveType::Sine),
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

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            let wave = if self.params.use_midi.value() {
                while let Some(event) = next_event {
                    if event.timing() > sample_id as u32 {
                        break;
                    }

                    match event {
                        NoteEvent::NoteOn { note, .. } => {
                            self.midi_note_id = note;
                            self.midi_note_freq = util::midi_note_to_freq(note);
                            self.envelope.note_on();
                            self.phase = 0.0;
                        }
                        NoteEvent::NoteOff { .. } => {
                            self.envelope.note_off();
                        }
                        _ => (),
                    }

                    next_event = context.next_event();
                }

                self.calculate_wave(self.midi_note_freq)
            } else {
                0.0
            };

            let volume = self.envelope.next_amp();

            for sample in channel_samples {
                *sample = wave * volume;
            }
        }

        ProcessStatus::Normal
    }
}

nih_export_vst3!(Serenity);
nih_export_clap!(Serenity);
