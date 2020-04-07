use derive_more::Constructor;
use math::round::floor;
use nannou::prelude::*;

mod macros;

pub trait Wave {
    fn sample(&self) -> f32;
    fn update_phase(&mut self, sample_rate: f64);
    fn mul_hz(&mut self, factor: f64);
    fn mod_hz(&mut self, factor: f64);
}

pub_struct!(
    struct WaveParams {
        hz: f64,
        volume: f32,
        phase: f64,
        hz0: f64,
    }
);

impl WaveParams {
    pub fn new(hz: f64, volume: f32) -> Self {
        WaveParams {
            hz,
            volume,
            phase: 0.0,
            hz0: hz,
        }
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.phase += self.hz / sample_rate;
        self.phase %= sample_rate;
    }

    fn mul_hz(&mut self, factor: f64) {
        self.hz *= factor;
        self.hz0 *= factor;
    }

    fn mod_hz(&mut self, factor: f64) {
        self.hz = self.hz0 * factor;
    }
}

pub struct ConstantWave;
impl Wave for ConstantWave {
    fn sample(&self) -> f32 {
        0.0
    }

    fn update_phase(&mut self, _sample_rate: f64) {}

    fn mul_hz(&mut self, _factor: f64) {}

    fn mod_hz(&mut self, _factor: f64) {}
}

basic_wave!(SineWave, |wave: &SineWave| {
    wave.0.volume * (TAU * wave.0.phase as f32).sin()
});

basic_wave!(SquareWave, |wave: &SquareWave| {
    let sine_wave = SineWave(WaveParams::new(wave.0.hz, wave.0.volume));
    let sine_amp = sine_wave.sample();
    if sine_amp > 0. {
        wave.0.volume
    } else {
        -wave.0.volume
    }
});

basic_wave!(RampWave, |wave: &RampWave| {
    wave.0.volume * (2. * (wave.0.phase - floor(0.5 + wave.0.phase, 0))) as f32
});

basic_wave!(SawWave, |wave: &SawWave| {
    let t = wave.0.phase - 0.5;
    wave.0.volume * (2. * (-t - floor(0.5 - t, 0))) as f32
});

basic_wave!(TriangleWave, |wave: &TriangleWave| {
    let t = wave.0.phase - 0.75;
    let saw_amp = (2. * (-t - floor(0.5 - t, 0))) as f32;
    2. * saw_amp.abs() - wave.0.volume
});

pub_struct!(
    #[derive(Constructor)]
    struct LerpWave {
        wave1: Box<dyn Wave + Send>,
        wave2: Box<dyn Wave + Send>,
        alpha: f32,
    }
);

impl Wave for LerpWave {
    fn sample(&self) -> f32 {
        (1. - self.alpha) * self.wave1.sample() + self.alpha * self.wave2.sample()
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.wave1.update_phase(sample_rate);
        self.wave2.update_phase(sample_rate);
    }

    fn mul_hz(&mut self, factor: f64) {
        self.wave1.mul_hz(factor);
        self.wave2.mul_hz(factor);
    }

    fn mod_hz(&mut self, factor: f64) {
        self.wave1.mod_hz(factor);
        self.wave2.mod_hz(factor);
    }
}

pub_struct!(
    /// Voltage Controlled Amplifier
    struct VCA {
        wave: Box<dyn Wave + Send>,
        control_voltage: Box<dyn Wave + Send>,
    }
);

impl Wave for VCA {
    fn sample(&self) -> f32 {
        self.wave.sample() * self.control_voltage.sample()
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.wave.update_phase(sample_rate);
        self.control_voltage.update_phase(sample_rate);
    }

    fn mul_hz(&mut self, factor: f64) {
        self.wave.mul_hz(factor);
    }
    fn mod_hz(&mut self, factor: f64) {
        self.wave.mod_hz(factor);
    }
}

pub_struct!(
    /// Voltage Controlled Oscillator
    struct VCO {
        wave: Box<dyn Wave + Send>,
        control_voltage: Box<dyn Wave + Send>,
    }
);

impl Wave for VCO {
    fn sample(&self) -> f32 {
        self.wave.sample()
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.wave.update_phase(sample_rate);
        self.control_voltage.update_phase(sample_rate);
        let factor = 2.0.powf(self.control_voltage.sample()) as f64;
        self.wave.mod_hz(factor);
    }

    fn mul_hz(&mut self, factor: f64) {
        self.wave.mul_hz(factor);
    }

    fn mod_hz(&mut self, factor: f64) {
        self.wave.mod_hz(factor);
    }
}

pub_struct!(
    struct ADSRWave {
        attack: f32,
        decay: f32,
        sustain_time: f32,
        sustain_level: f32,
        release: f32,
        time: f64,
    }
);

impl ADSRWave {
    fn adsr(&self, t: f32) -> f32 {
        let a = self.attack;
        let d = self.decay;
        let s = self.sustain_time;
        let r = self.release;
        let sl = self.sustain_level;
        match t {
            x if x < a => t / a,
            x if x < a + d => 1.0 + (t - a) * (sl - 1.0) / d,
            x if x < a + d + s => sl,
            x if x < a + d + s + r => sl - (t - a - d - s) * sl / r,
            _ => 0.0,
        }
    }
}

impl Wave for ADSRWave {
    fn sample(&self) -> f32 {
        self.adsr(self.time as f32)
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.time += 1. / sample_rate;
    }

    fn mul_hz(&mut self, _factor: f64) {}
    fn mod_hz(&mut self, _factor: f64) {}
}

pub_struct!(struct WeightedWave(Box<dyn Wave + Send>, f32));

pub_struct!(
    struct AvgWave {
        waves: Vec<WeightedWave>,
    }
);

impl Wave for AvgWave {
    fn sample(&self) -> f32 {
        let total_weight = self.waves.iter().fold(0.0, |acc, x| acc + x.1);
        self.waves
            .iter()
            .fold(0.0, |acc, x| acc + x.1 * x.0.sample())
            / total_weight
    }

    fn update_phase(&mut self, sample_rate: f64) {
        for wave in self.waves.iter_mut() {
            wave.0.update_phase(sample_rate);
        }
    }

    fn mul_hz(&mut self, factor: f64) {
        for wave in self.waves.iter_mut() {
            wave.0.mul_hz(factor);
        }
    }

    fn mod_hz(&mut self, factor: f64) {
        for wave in self.waves.iter_mut() {
            wave.0.mod_hz(factor);
        }
    }
}
