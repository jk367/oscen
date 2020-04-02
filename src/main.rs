use core::cmp::Ordering;
use core::time::Duration;
use crossbeam::crossbeam_channel::{unbounded, Receiver, Sender};
use derive_more::Constructor;
use math::round::floor;
use nannou::prelude::*;
use nannou_audio as audio;
use nannou_audio::Buffer;
use std::f64::consts::PI;

fn main() {
    nannou::app(model).update(update).run();
}

trait Wave {
    fn sample(&self) -> f32;
    fn update_phase(&mut self, sample_rate: f64);
    fn mult_hz(&mut self, factor: f64);
}

#[derive(Constructor)]
struct WaveParams {
    hz: f64,
    volume: f32,
    phase: f64,
}

impl WaveParams {
    fn update_phase(&mut self, sample_rate: f64) {
        self.phase += self.hz / sample_rate;
        self.phase %= sample_rate;
    }

    fn mult_hz(&mut self, factor: f64) {
        self.hz *= factor;
    }
}

struct SineWave(WaveParams);

impl SineWave {
    fn new(hz: f64, volume: f32, phase: f64) -> Self {
        SineWave(WaveParams::new(hz, volume, phase))
    }
}

impl Wave for SineWave {
    fn sample(&self) -> f32 {
        self.0.volume * (2. * PI * self.0.phase).sin() as f32
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.0.update_phase(sample_rate)
    }

    fn mult_hz(&mut self, factor: f64) {
        self.0.mult_hz(factor);
    }
}

struct SquareWave(WaveParams);

impl SquareWave {
    fn new(hz: f64, volume: f32, phase: f64) -> Self {
        SquareWave(WaveParams::new(hz, volume, phase))
    }
}

impl Wave for SquareWave {
    fn sample(&self) -> f32 {
        let sine_wave = SineWave(WaveParams::new(self.0.hz, self.0.volume, self.0.phase));
        let sine_amp = sine_wave.sample();
        if sine_amp > 0. {
            self.0.volume
        } else {
            -self.0.volume
        }
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.0.update_phase(sample_rate)
    }

    fn mult_hz(&mut self, factor: f64) {
        self.0.mult_hz(factor);
    }
}

struct RampWave(WaveParams);

impl RampWave {
    fn new(hz: f64, volume: f32, phase: f64) -> Self {
        RampWave(WaveParams::new(hz, volume, phase))
    }
}

impl Wave for RampWave {
    fn sample(&self) -> f32 {
        self.0.volume * (2. * (self.0.phase - floor(0.5 + self.0.phase, 0))) as f32
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.0.update_phase(sample_rate);
    }

    fn mult_hz(&mut self, factor: f64) {
        self.0.mult_hz(factor);
    }
}

struct SawWave(WaveParams);

impl SawWave {
    fn new(hz: f64, volume: f32, phase: f64) -> Self {
        SawWave(WaveParams::new(hz, volume, phase))
    }
}

impl Wave for SawWave {
    fn sample(&self) -> f32 {
        let t = self.0.phase - 0.5;
        self.0.volume * (2. * (-t - floor(0.5 - t, 0))) as f32
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.0.update_phase(sample_rate);
    }

    fn mult_hz(&mut self, factor: f64) {
        self.0.mult_hz(factor);
    }
}

struct TriangleWave(WaveParams);

impl TriangleWave {
    fn new(hz: f64, volume: f32, phase: f64) -> Self {
        TriangleWave(WaveParams::new(hz, volume, phase))
    }
}

impl Wave for TriangleWave {
    fn sample(&self) -> f32 {
        let t = self.0.phase - 0.75;
        let saw_amp = (2. * (-t - floor(0.5 - t, 0))) as f32;
        2. * saw_amp.abs() - self.0.volume
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.0.update_phase(sample_rate);
    }

    fn mult_hz(&mut self, factor: f64) {
        self.0.mult_hz(factor);
    }
}

#[derive(Constructor)]
struct LerpWave {
    wave1: Box<dyn Wave + Send>,
    wave2: Box<dyn Wave + Send>,
    alpha: f32,
}

impl Wave for LerpWave {
    fn sample(&self) -> f32 {
        (1. - self.alpha) * self.wave1.sample() + self.alpha * self.wave2.sample()
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.wave1.update_phase(sample_rate);
        self.wave2.update_phase(sample_rate);
    }

    fn mult_hz(&mut self, factor: f64) {
        self.wave1.mult_hz(factor);
        self.wave2.mult_hz(factor);
    }
}

struct MultWave {
    base_wave: Box<dyn Wave + Send>,
    mod_wave: Box<dyn Wave + Send>,
}

impl Wave for MultWave {
    fn sample(&self) -> f32 {
        self.base_wave.sample() * self.mod_wave.sample()
    }

    fn update_phase(&mut self, sample_rate: f64) {
        self.base_wave.update_phase(sample_rate);
        self.mod_wave.update_phase(sample_rate);
    }

    fn mult_hz(&mut self, factor: f64) {
        self.base_wave.mult_hz(factor);
        // only change the frequency of the base wave.
    }
}

struct AvgWave {
    waves: Vec<Box<dyn Wave + Send>>,
}

impl Wave for AvgWave {
    fn sample(&self) -> f32 {
        self.waves.iter().fold(0.0, |acc, x| acc + x.sample()) / self.waves.len() as f32
    }

    fn update_phase(&mut self, sample_rate: f64) {
        for wave in self.waves.iter_mut() {
            wave.update_phase(sample_rate);
        }
    }

    fn mult_hz(&mut self, factor: f64) {
        for wave in self.waves.iter_mut() {
            wave.mult_hz(factor);
        }
    }
}

#[derive(Constructor)]
struct Model {
    stream: audio::Stream<Synth>,
    receiver: Receiver<f32>,
    amps: Vec<f32>,
    max_amp: f32,
}

#[derive(Constructor)]
struct Synth {
    voice: Box<dyn Wave + Send>,
    sender: Sender<f32>,
}

fn model(app: &App) -> Model {
    let (sender, receiver) = unbounded();

    // Create a window to receive key pressed events.
    app.set_loop_mode(LoopMode::Rate {
        update_interval: Duration::from_millis(1),
    });
    app.new_window()
        .key_pressed(key_pressed)
        .view(view)
        .build()
        .unwrap();
    // Initialise the audio API so we can spawn an audio stream.
    let audio_host = audio::Host::new();
    // Initialise the state that we want to live on the audio thread.
    let wave1 = Box::new(SineWave::new(130.81, 0.5, 0.0));
    let wave2 = Box::new(SquareWave::new(260., 0.5, 0.0));
    let wave3 = Box::new(TriangleWave::new(196.00, 0.5, 0.0));
    // let lerp_voice = LerpWave {
    //     wave1,
    //     wave2,
    //     alpha: 0.5,
    // };
    // let avg_voice = AvgWave {
    //     waves: vec![wave1, wave2, wave3],
    // };
    let mult_wave = MultWave { base_wave: wave1, mod_wave: wave2};
    let model = Synth {
        voice: Box::new(mult_wave),
        sender,
    };
    let stream = audio_host
        .new_output_stream(model)
        .render(audio)
        .build()
        .unwrap();
    Model {
        stream,
        receiver,
        amps: vec![],
        max_amp: 0.,
    }
}

// A function that renders the given `Audio` to the given `Buffer`.
// In this case we play a simple sine wave at the audio's current frequency in `hz`.
fn audio(synth: &mut Synth, buffer: &mut Buffer) {
    let sample_rate = buffer.sample_rate() as f64;
    for frame in buffer.frames_mut() {
        let mut amp = 0.;
        amp += synth.voice.sample();
        synth.voice.update_phase(sample_rate);
        for channel in frame {
            *channel = amp;
        }
        synth.sender.send(amp).unwrap();
    }
}

fn key_pressed(_app: &App, model: &mut Model, key: Key) {
    model.max_amp = 0.;
    let change_hz = |i| {
        model
            .stream
            .send(move |synth| {
                let factor = 2.0.powf(i / 12.);
                synth.voice.mult_hz(factor);
            })
            .unwrap();
    };
    match key {
        // Pause or unpause the audio when Space is pressed.
        Key::Space => {
            if model.stream.is_playing() {
                model.stream.pause().unwrap();
            } else {
                model.stream.play().unwrap();
            }
        }
        // Raise the frequency when the up key is pressed.
        Key::Up => change_hz(1.),
        Key::Down => change_hz(-1.),
        _ => {}
    }
}

fn update(_app: &App, model: &mut Model, _update: Update) {
    let amps: Vec<f32> = model.receiver.try_iter().collect();
    let clone = amps.clone();

    // find max amplitude in waveform
    let max = amps.iter().max_by(|x, y| {
        if x > y {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    });

    // store if it's greater than the previously stored max
    if max.is_some() && *max.unwrap() > model.max_amp {
        model.max_amp = *max.unwrap();
    }

    model.amps = clone;
}

fn view(app: &App, model: &Model, frame: Frame) {
    let mut shifted: Vec<f32> = vec![];
    let mut iter = model.amps.iter().peekable();

    let mut i = 0;
    while iter.len() > 0 {
        let amp = iter.next().unwrap_or(&0.);
        if amp.abs() < 0.01 && **iter.peek().unwrap_or(&amp) > *amp {
            shifted = model.amps[i..].to_vec();
            break;
        }
        i += 1;
    }

    let l = 600;
    let mut points: Vec<Point2> = vec![];
    for (i, amp) in shifted.iter().enumerate() {
        if i == l {
            break;
        }
        points.push(pt2(i as f32, amp * 80.));
    }

    // only draw if we got enough info back from the audio thread
    if points.len() == 600 {
        let draw = app.draw();
        frame.clear(BLACK);
        draw.path()
            .stroke()
            .weight(1.)
            .points(points)
            .x_y(-300., 0.);

        draw.to_frame(app, &frame).unwrap();
    }
}
