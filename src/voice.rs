use nih_plug::util;

use crate::{WaveType, envelope::Envelope, oscillator::Oscillator};

pub struct Voice {
    pub oscillators: Vec<Oscillator>,
    pub envelope: Envelope,
    pub midi_note_id: Option<u8>, // None if voice is not in use
    pub frequency: f32,
    pub age: u8, // only 128 midi notes so u8 should be fine
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            oscillators: vec![Oscillator::default()],
            envelope: Envelope::default(),
            midi_note_id: None,
            frequency: 0.0,
            age: 0,
        }
    }
}

impl Voice {
    pub fn calculate_wave(
        &mut self,
        wave_type: WaveType,
        spread: f32,
        pitch_bend: f32,
    ) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;

        let pitch_bend_range = 1200.0; // i could probably make this user configurable in the future, not sure how other synths do it, should look into it
        let pitch_cents = (pitch_bend - 0.5) * 2.0 * pitch_bend_range;

        let oscillator_count = self.oscillators.len();

        for (i, oscillator) in self.oscillators.iter_mut().enumerate() {
            let (offset_cents, pan) = if oscillator_count == 1 {
                (0.0, 0.0)
            } else {
                let temp = i as f32 / (oscillator_count - 1) as f32;
                (temp * (spread * 2.0) - spread, temp * 2.0 - 1.0)
            };

            let detuned_freq = self.frequency * 2.0_f32.powf((offset_cents + pitch_cents) / 1200.0);
            let sample = oscillator.calculate_wave(wave_type, detuned_freq);

            // Constant panning law - https://www.cs.cmu.edu/~music/icm-online/readings/panlaws/
            let amp = self.envelope.next_amp();
            left += sample * ((1.0 - pan) / 2.0).sqrt() * 2.0_f32.sqrt() * amp;
            right += sample * ((1.0 + pan) / 2.0).sqrt() * 2.0_f32.sqrt() * amp;
        }

        (left, right)
    }

    pub fn set_voice(&mut self, note_id: u8) {
        self.midi_note_id = Some(note_id);
        self.frequency = util::midi_note_to_freq(note_id);
        self.age = 1;
        self.envelope.note_on();
    }

    pub fn voice_off(&mut self) {
        self.age = 0;
        self.midi_note_id = None;
        self.frequency = 0.0;
    }
}
