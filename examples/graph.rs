// #![allow(dead_code)]

use core::cmp::Ordering;
use core::time::Duration;
use crossbeam::crossbeam_channel::{unbounded, Receiver, Sender};
use math::round::floor;
use midir::{Ignore, MidiInput};
use nannou::prelude::*;
use nannou::ui::prelude::*;
use nannou_audio as audio;
use nannou_audio::Buffer;
use pitch_calc::calc::hz_from_step;
use std::any::*;
use std::error::Error;
use std::f64::consts::PI;
use std::{
    io::{stdin, stdout, Write},
    thread,
};
use swell::dsp::*;

pub const TAU64: f64 = 2.0 * PI;
pub const TAU32: f32 = TAU64 as f32;

fn main() {
    nannou::app(model).update(update).run();
}
pub trait SignalG: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn signal(&mut self, graph: &Graph, sample_rate: f64) -> f64;
}

type SS = dyn SignalG + Send;

#[derive(Clone)]
pub enum Input {
    Variable(usize),
    Constant(f64),
}

pub struct Node {
    pub module: ArcMutex<SS>,
    pub output: f64,
}

impl Node {
    fn new(sig: ArcMutex<SS>) -> Self {
        Node {
            module: sig,
            output: 0.0,
        }
    }
}

pub struct Graph(pub Vec<Node>);

impl Graph {
    fn new(ws: Vec<ArcMutex<SS>>) -> Self {
        let mut ns: Vec<Node> = Vec::new();
        for s in ws {
            ns.push(Node::new(s));
        }
        Graph(ns)
    }

    fn output(&self, n: usize) -> f64 {
        self.0[n].output
    }

    fn play(&mut self, sample_rate: f64) -> f64 {
        let mut outs: Vec<f64> = Vec::new();
        for node in self.0.iter() {
            outs.push(node.module.lock().unwrap().signal(&self, sample_rate));
        }
        for (i, node) in self.0.iter_mut().enumerate() {
            node.output = outs[i];
        }
        self.0[self.0.len() - 1].output
    }
}

#[derive(Clone)]
pub struct SineOscG {
    pub hz: Input,
    pub amplitude: Input,
    pub phase: Input,
}

impl SineOscG {
    fn new(hz: Input) -> Self {
        SineOscG {
            hz,
            amplitude: Input::Constant(1.0),
            phase: Input::Constant(0.0),
        }
    }
}

impl SignalG for SineOscG {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn signal(&mut self, graph: &Graph, sample_rate: f64) -> f64 {
        let hz = match self.hz {
            Input::Variable(n) => graph.output(n),
            Input::Constant(hz) => hz,
        };
        let amplitude = match self.amplitude {
            Input::Variable(n) => graph.output(n),
            Input::Constant(amp) => amp,
        };
        let phase = match self.phase {
            Input::Variable(n) => graph.output(n),
            Input::Constant(ph) => ph,
        };
        self.phase = match &self.phase {
            Input::Constant(p) => {
                let mut ph = p + hz / sample_rate;
                ph %= sample_rate;
                Input::Constant(ph)
            }
            Input::Variable(x) => Input::Variable(*x),
        };
        amplitude * (TAU64 * phase).sin()
    }
}
pub struct Osc01 {
    pub hz: Input,
    pub phase: Input,
}

impl Osc01 {
    fn new(hz: Input) -> Self {
        Osc01 {
            hz,
            phase: Input::Constant(0.0),
        }
    }
}

impl SignalG for Osc01 {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn signal(&mut self, graph: &Graph, sample_rate: f64) -> f64 {
        let hz = match self.hz {
            Input::Variable(n) => graph.output(n),
            Input::Constant(hz) => hz,
        };
        let phase = match self.phase {
            Input::Variable(n) => graph.output(n),
            Input::Constant(ph) => ph,
        };
        self.phase = match &self.phase {
            Input::Constant(p) => {
                let mut ph = p + hz / sample_rate;
                ph %= sample_rate;
                Input::Constant(ph)
            }
            Input::Variable(x) => Input::Variable(*x),
        };
        0.5 * ((TAU64 * phase).sin() + 1.0)
    }
}



#[derive(Clone)]
pub struct SquareOscG {
    pub hz: Input,
    pub amplitude: Input,
    pub phase: Input,
    pub duty_cycle: f64,
}

impl SquareOscG {
    fn new(hz: Input) -> Self {
        SquareOscG {
            hz,
            amplitude: Input::Constant(1.0),
            phase: Input::Constant(0.0),
            duty_cycle: 0.5,
        }
    }
}

impl SignalG for SquareOscG {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn signal(&mut self, graph: &Graph, sample_rate: f64) -> f64 {
        let hz = match self.hz {
            Input::Variable(n) => graph.output(n),
            Input::Constant(hz) => hz,
        };
        let amplitude = match self.amplitude {
            Input::Variable(n) => graph.output(n),
            Input::Constant(amp) => amp,
        };
        let phase = match self.phase {
            Input::Variable(n) => graph.output(n),
            Input::Constant(ph) => ph,
        };
        self.phase = match &self.phase {
            Input::Constant(p) => {
                let mut ph = p + hz / sample_rate;
                ph %= sample_rate;
                Input::Constant(ph)
            }
            Input::Variable(x) => Input::Variable(*x),
        };
        let t = phase - floor(phase, 0);
        if t < 0.001 {
            0.0
        } else if t <= self.duty_cycle {
            amplitude
        } else {
            -amplitude
        }
    }
}

pub struct LerpG {
    wave1: usize,
    wave2: usize,
    alpha: Input,
}

impl LerpG {
    fn new(wave1: usize, wave2: usize) -> Self {
        LerpG {
            wave1,
            wave2,
            alpha: Input::Constant(0.5),
        }
    }
}

impl SignalG for LerpG {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn signal(&mut self, graph: &Graph, _sample_rate: f64) -> f64 {
        let alpha = match self.alpha {
            Input::Constant(a) => a,
            Input::Variable(n) => graph.output(n),
        };
        alpha * graph.output(self.wave1) + (1.0 - alpha) * graph.output(self.wave2)
    }
}

struct Synth {
    voice: Graph,
    sender: Sender<f32>,
}

struct Model {
    ui: Ui,
    ids: Ids,
    alpha: f64,
    // attack: Amp,
    // decay: Amp,
    // sustain_time: Amp,
    // sustain_level: Amp,
    // release: Amp,
    stream: audio::Stream<Synth>,
    receiver: Receiver<f32>,
    midi_receiver: Receiver<Vec<u8>>,
    amps: Vec<f32>,
    max_amp: f32,
}

fn model(app: &App) -> Model {
    let (sender, receiver) = unbounded();
    let (midi_sender, midi_receiver) = unbounded();

    thread::spawn(|| match listen_midi(midi_sender) {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    });

    // Create a window to receive key pressed events.
    app.set_loop_mode(LoopMode::Rate {
        update_interval: Duration::from_millis(1),
    });

    let _window = app.new_window().size(900, 620).view(view).build().unwrap();

    let mut ui = app.new_ui().build().unwrap();

    let ids = Ids {
        alpha: ui.generate_widget_id(),
        // attack: ui.generate_widget_id(),
        // decay: ui.generate_widget_id(),
        // sustain_time: ui.generate_widget_id(),
        // sustain_level: ui.generate_widget_id(),
        // release: ui.generate_widget_id(),
    };
    let audio_host = audio::Host::new();

    let sinewave = SineOscG::new(Input::Constant(220.0));
    let squarewave = SquareOscG::new(Input::Constant(220.0));
    let osc01 = Osc01::new(Input::Constant(1.0));
    let mut lerp = LerpG::new(0, 1);
    lerp.alpha = Input::Variable(2);

    let voice = Graph::new(vec![arc(sinewave), arc(squarewave), arc(osc01), arc(lerp)]);
    let synth = Synth { voice, sender };
    let stream = audio_host
        .new_output_stream(synth)
        .render(audio)
        .build()
        .unwrap();

    Model {
        ui,
        ids,
        alpha: 0.5,
        // attack: 0.2,
        // decay: 0.1,
        // sustain_time: 0.2,
        // sustain_level: 0.5,
        // release: 0.2,
        stream,
        receiver,
        midi_receiver,
        amps: vec![],
        max_amp: 0.,
    }
}

struct Ids {
    alpha: widget::Id,
    // attack: widget::Id,
    // decay: widget::Id,
    // sustain_time: widget::Id,
    // sustain_level: widget::Id,
    // release: widget::Id,
}

fn listen_midi(midi_sender: Sender<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.port_count();
    let in_port = match in_ports {
        0 => return Err("no input port found".into()),
        1 => {
            println!(
                "Choosing the only available input port: {}",
                midi_in.port_name(0).unwrap()
            );
            0
        }
        _ => {
            println!("\nAvailable input ports:");
            for i in 0..in_ports {
                println!("{}: {}", i, midi_in.port_name(i).unwrap());
            }
            print!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            input.trim().parse::<usize>()?
        }
    };

    println!("\nOpening connection");

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_, message, _| {
            midi_sender.send(message.to_vec()).unwrap();
        },
        (),
    )?;

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");
    Ok(())
}

fn audio(synth: &mut Synth, buffer: &mut Buffer) {
    let sample_rate = buffer.sample_rate() as f64;
    for frame in buffer.frames_mut() {
        let mut amp = 0.;
        amp += synth.voice.play(sample_rate);
        for channel in frame {
            *channel = amp as f32;
        }
        synth.sender.send(amp as f32).unwrap();
    }
}

fn update(_app: &App, model: &mut Model, _update: Update) {
    let midi_messages: Vec<Vec<u8>> = model.midi_receiver.try_iter().collect();
    for message in midi_messages {
        if message.len() == 3 {
            if message[0] == 144 {
                model
                    .stream
                    .send(move |synth| {
                        let step = message[1];
                        let hz = hz_from_step(step as f32) as f64;
                        if let Some(v) = synth.voice.0[0]
                            .module
                            .lock()
                            .unwrap()
                            .as_any_mut()
                            .downcast_mut::<SineOscG>()
                        {
                            v.hz = Input::Constant(hz);
                        }
                        if let Some(v) = synth.voice.0[1]
                            .module
                            .lock()
                            .unwrap()
                            .as_any_mut()
                            .downcast_mut::<SquareOscG>()
                        {
                            v.hz = Input::Constant(hz);
                        }
                        // synth.voice.on();
                    })
                    .unwrap();
            }
        }
    }

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

    //UI
    let ui = &mut model.ui.set_widgets();

    fn slider(val: f32, min: f32, max: f32) -> widget::Slider<'static, f32> {
        widget::Slider::new(val, min, max)
            .w_h(200.0, 30.0)
            .label_font_size(15)
            .rgb(0.1, 0.2, 0.5)
            .label_rgb(1.0, 1.0, 1.0)
            .border(0.0)
    }

    for value in slider(model.alpha as f32, 0.0, 1.0)
        .top_left_with_margin(20.0)
        .label("Alpha")
        .set(model.ids.alpha, ui)
    {
        model.alpha = value as f64;
        model
            .stream
            .send(move |synth| {
                if let Some(v) = synth.voice.0[2]
                    .module
                    .lock()
                    .unwrap()
                    .as_any_mut()
                    .downcast_mut::<LerpG>()
                {
                    v.alpha = Input::Constant(value as f64);
                }
            })
            .unwrap();
    }

    // for value in slider(model.attack, 0.0, 1.0)
    //     .down(20.)
    //     .label("Attack")
    //     .set(model.ids.attack, ui)
    // {
    //     model.attack = value;
    //     model
    //         .stream
    //         .send(move |synth| {
    //             synth.voice.attack = value;
    //         })
    //         .unwrap();
    // }

    // for value in slider(model.decay, 0.0, 1.0)
    //     .down(20.)
    //     .label("Decay")
    //     .set(model.ids.decay, ui)
    // {
    //     model.decay = value;
    //     model
    //         .stream
    //         .send(move |synth| {
    //             synth.voice.decay = value;
    //         })
    //         .unwrap();
    // }

    // for value in slider(model.sustain_time, 0.0, 10.0)
    //     .down(20.)
    //     .label("Sustain Time")
    //     .set(model.ids.sustain_time, ui)
    // {
    //     model.sustain_time = value;
    //     model
    //         .stream
    //         .send(move |synth| {
    //             synth.voice.sustain_time = value;
    //         })
    //         .unwrap();
    // }

    // for value in slider(model.sustain_level, 0.0, 1.0)
    //     .down(20.)
    //     .label("Sustain Level")
    //     .set(model.ids.sustain_level, ui)
    // {
    //     model.sustain_level = value;
    //     model
    //         .stream
    //         .send(move |synth| {
    //             synth.voice.sustain_level = value;
    //         })
    //         .unwrap();
    // }

    // for value in slider(model.release, 0.0, 1.0)
    //     .down(20.)
    //     .label("Release")
    //     .set(model.ids.release, ui)
    // {
    //     model.release = value;
    //     model
    //         .stream
    //         .send(move |synth| {
    //             synth.voice.release = value;
    //         })
    //         .unwrap();
    // }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let c = rgb(9. / 255., 9. / 255., 44. / 255.);
    draw.background().color(c);
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
        points.push(pt2(i as f32, amp * 120.));
    }

    // only draw if we got enough info back from the audio thread
    if points.len() == 600 {
        draw.path()
            .stroke()
            .weight(2.)
            .points(points)
            .color(CORNFLOWERBLUE)
            .x_y(-200., 0.);

        draw.to_frame(app, &frame).unwrap();
    }

    // Draw the state of the `Ui` to the frame.
    model.ui.draw_to_frame(app, &frame).unwrap();
}
