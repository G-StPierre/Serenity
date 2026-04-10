use nih_plug::{
    params::{FloatParam, Params},
    prelude::FloatRange,
};

#[derive(Params)]
pub struct EnvelopeParams {
    #[id = "attack (ms)"]
    pub attack: FloatParam,
    #[id = "Decay (ms)"]
    pub decay: FloatParam,
    #[id = "Sustain"]
    pub sustain: FloatParam,
    #[id = "Release"]
    pub release: FloatParam,
}

impl Default for EnvelopeParams {
    fn default() -> Self {
        EnvelopeParams {
            attack: FloatParam::new(
                "Attack",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10000.0,
                },
            ),
            decay: FloatParam::new(
                "Decay",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10000.0,
                },
            ),
            sustain: FloatParam::new("Sustain", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            release: FloatParam::new(
                "Release",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10000.0,
                },
            ),
        }
    }
}

enum EnvelopeState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

struct EnvelopeSteps {
    attack: f32,
    decay: f32,
    release: f32,
}

impl Default for EnvelopeSteps {
    fn default() -> Self {
        EnvelopeSteps {
            attack: 1.0,
            decay: 1.0,
            release: 1.0,
        }
    }
}

pub struct Envelope {
    state: EnvelopeState,
    sample_rate: f32,
    sustain: f32,
    pub amp_level: f32,
    envelope_steps: EnvelopeSteps,
}

impl Envelope {
    pub fn default() -> Self {
        Envelope {
            state: EnvelopeState::Idle,
            sample_rate: 44100.0,
            sustain: 1.0,
            amp_level: 0.0,
            envelope_steps: EnvelopeSteps::default(),
        }
    }

    pub fn update_params(&mut self, params: &EnvelopeParams) {
        self.envelope_steps.attack = self.calculate_step(params.attack.value(), self.sample_rate);
        self.envelope_steps.decay = self.calculate_step(params.decay.value(), self.sample_rate);
        self.envelope_steps.release = self.calculate_step(params.release.value(), self.sample_rate);
        self.sustain = params.sustain.value();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn calculate_step(&mut self, time: f32, sample_rate: f32) -> f32 {
        let time = time.max(2.0); // Stops the instant jump - Read more in this forum - https://www.kvraudio.com/forum/viewtopic.php?t=255852

        1.0 / (sample_rate * (time / 1000.0))
    }

    pub fn next_amp(&mut self) -> f32 {
        match self.state {
            EnvelopeState::Idle => self.amp_level = 0.0,
            EnvelopeState::Attack => {
                if self.amp_level < 1.0 {
                    self.amp_level += self.envelope_steps.attack;
                } else {
                    self.state = EnvelopeState::Decay
                }
            }
            EnvelopeState::Decay => {
                if self.amp_level > self.sustain {
                    self.amp_level -= self.envelope_steps.decay;

                    if self.amp_level < self.sustain {
                        // In case decay is zero ms which sets the step to 1 which causes my amp to be zero
                        self.amp_level = self.sustain;
                        self.state = EnvelopeState::Sustain;
                    }
                } else {
                    self.state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {}
            EnvelopeState::Release => {
                if self.amp_level > 0.0 {
                    self.amp_level -= self.envelope_steps.release;
                } else {
                    self.amp_level = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
        }
        self.amp_level
    }

    pub fn note_on(&mut self) {
        self.amp_level = 0.0;
        self.state = EnvelopeState::Attack;
    }

    pub fn note_off(&mut self) {
        self.state = EnvelopeState::Release;
    }
}
