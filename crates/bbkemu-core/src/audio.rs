//! Audio emulation

/// Audio channel state
pub struct AudioChannel {
    /// Frequency in Hz
    pub frequency: u16,
    /// Duration remaining in samples
    pub duration: u32,
    /// Current phase (0.0 to 1.0)
    phase: f32,
    /// Whether channel is active
    pub active: bool,
}

/// Audio system
pub struct Audio {
    /// Sample rate
    sample_rate: u32,
    /// Tone channel
    tone: AudioChannel,
    /// Output buffer
    buffer: Vec<f32>,
}

impl Audio {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            tone: AudioChannel {
                frequency: 0,
                duration: 0,
                phase: 0.0,
                active: false,
            },
            buffer: Vec::new(),
        }
    }

    /// Play a tone
    pub fn play_tone(&mut self, freq: u16, duration_ms: u16) {
        self.tone.frequency = freq;
        self.tone.duration = (self.sample_rate * duration_ms as u32) / 1000;
        self.tone.phase = 0.0;
        self.tone.active = true;
    }

    /// Stop all sound
    pub fn stop(&mut self) {
        self.tone.active = false;
        self.tone.duration = 0;
    }

    /// Generate audio samples
    pub fn mix_samples(&mut self, buf: &mut [f32]) {
        for sample in buf.iter_mut() {
            *sample = 0.0;

            if self.tone.active && self.tone.duration > 0 {
                // Generate square wave
                let value = if self.tone.phase < 0.5 { 0.3 } else { -0.3 };
                *sample += value;

                // Advance phase
                self.tone.phase += self.tone.frequency as f32 / self.sample_rate as f32;
                if self.tone.phase >= 1.0 {
                    self.tone.phase -= 1.0;
                }

                // Decrease duration
                self.tone.duration -= 1;
                if self.tone.duration == 0 {
                    self.tone.active = false;
                }
            }
        }
    }

    /// Get the output buffer
    pub fn buffer(&self) -> &[f32] {
        &self.buffer
    }
}
