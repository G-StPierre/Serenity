use std::f32::consts;

use crate::WaveType;

pub struct Oscillator {
    phase: f32,
    frequency: f32,
    detune: f32,
    sample_rate: f32,
}

impl Default for Oscillator {
    fn default() -> Self {
        Oscillator {
            phase: 0.0,
            frequency: 0.0,
            detune: 0.0,
            sample_rate: 44100.0,
        }
    }
}

impl Oscillator {
    pub fn calculate_wave(&mut self, wave_type: WaveType, frequency: f32) -> f32 {
        match wave_type {
            // !!! Account for detune here
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
