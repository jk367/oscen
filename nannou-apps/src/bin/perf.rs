use nannou::prelude::*;
use nannou_audio as audio;
use nannou_audio::Buffer;
use oscen::ops::*;
use oscen::osc::*;
use oscen::rack::*;

fn main() {
    nannou::app(model).run();
}

struct Model {
    _stream: audio::Stream<Synth>,
}

struct Synth {
    rack: Rack,
    controls: Box<Controls>,
    state: Box<State>,
    outputs: Box<Outputs>,
}

fn model(app: &App) -> Model {
    app.new_window().size(250, 250).build().unwrap();
    let audio_host = audio::Host::new();
    let mut rack = Rack::new();
    let mut controls = Controls::new();
    let mut state = State::new();
    let outputs = Outputs::new();
    let mut oscs = vec![];
    let osc = OscBuilder::new(square_osc)
        .hz(440)
        .rack(&mut rack, &mut controls, &mut state);
    oscs.push(osc.tag());
    let mut builder = triangle_wave(32);
    builder.hz(220).lanczos(false);
    let osc = builder.rack(&mut rack, &mut controls);
    oscs.push(osc.tag());

    let union = UnionBuilder::new(oscs).rack(&mut rack, &mut controls);

    let synth = Synth {
        rack,
        controls: Box::new(controls),
        state: Box::new(state),
        outputs: Box::new(outputs),
    };
    let stream = audio_host
        .new_output_stream(synth)
        .render(audio)
        .build()
        .unwrap();
    Model { _stream: stream }
}

fn audio(synth: &mut Synth, buffer: &mut Buffer) {
    let sample_rate = buffer.sample_rate() as Real;
    for frame in buffer.frames_mut() {
        let amp = synth.rack.mono(
            &mut synth.controls,
            &mut synth.state,
            &mut synth.outputs,
            sample_rate,
        ) as f32;
        for channel in frame {
            *channel = amp;
        }
    }
}
