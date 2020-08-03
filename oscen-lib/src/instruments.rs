use super::{
    envelopes::Adsr,
    filters::Lpf,
    operators::{Delay, Mixer, Product},
    signal::*,
};
use crate::{as_any_mut, gate, std_signal};
use std::any::Any;

#[derive(Clone)]
pub struct WaveGuide {
    tag: Tag,
    burst: Tag,
    hz: In,
    cutoff_freq: In,
    wet_decay: In,
    input: ArcMutex<Link>,
    envelope: ArcMutex<Adsr>,
    lpf: ArcMutex<Lpf>,
    delay: ArcMutex<Delay>,
    mixer: ArcMutex<Mixer>,
    rack: Rack,
}

impl WaveGuide {
    pub fn new(id_gen: &mut IdGen, burst: Tag) -> Self {
        let mut rack = Rack::new();
        let mut id = IdGen::new();

        let input = Link::new(&mut id).wrap();
        rack.append(input.clone());

        // Adsr
        let envelope = Adsr::new(&mut id, 0.2, 0.2, 0.2)
            .attack(0.001)
            .decay(0)
            .sustain(0)
            .release(0.001)
            .wrap();
        rack.append(envelope.clone());

        // Exciter: gated noise
        let exciter = Product::new(&mut id, vec![input.tag(), envelope.tag()]).wrap();
        rack.append(exciter.clone());

        // Feedback loop
        let mut mixer = Mixer::new(&mut id, vec![]).build();
        let delay = Delay::new(&mut id, mixer.tag(), (0.02).into()).wrap();

        let cutoff_freq = 2000;
        let lpf = Lpf::new(&mut id, delay.tag()).cutoff_freq(cutoff_freq).wrap();

        let wet_decay = 0.95;
        let mixer = mixer
            .waves(vec![exciter.tag(), lpf.tag()])
            .levels(vec![1.0, wet_decay])
            .wrap();

        rack.append(lpf.clone());
        rack.append(delay.clone());
        rack.append(mixer.clone());

        WaveGuide {
            tag: id_gen.id(),
            burst,
            hz: 440.into(),
            cutoff_freq: cutoff_freq.into(),
            wet_decay: wet_decay.into(),
            input,
            envelope,
            lpf,
            delay,
            mixer,
            rack,
        }
    }

    pub fn hz<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.hz = arg.into();
        self
    }

    pub fn on(&mut self) {
        self.envelope.lock().on();
    }

    pub fn off(&mut self) {
        self.envelope.lock().off();
    }

    pub fn attack<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.envelope.lock().attack(arg);
        self
    }

    pub fn decay<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.envelope.lock().decay(arg);
        self
    }

    pub fn sustain<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.envelope.lock().sustain(arg);
        self
    }

    pub fn release<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.envelope.lock().release(arg);
        self
    }

    pub fn cutoff_freq<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.lpf.lock().cutoff_freq(arg);
        self
    }

    pub fn wet_decay<T: Into<In>>(&mut self, arg: T) -> &mut Self {
        self.mixer.lock().level_nth(1, arg.into());
        self
    }
}

impl Builder for WaveGuide {}

gate!(WaveGuide);

impl Signal for WaveGuide {
    std_signal!();
    fn signal(&mut self, rack: &Rack, sample_rate: Real) -> Real {
        let input = rack.output(self.burst);
        self.input.lock().value(input);
        let dt = 1.0 / f64::max(1.0, In::val(&rack, self.hz));
        self.delay.lock().delay_time(dt);
        self.rack.signal(sample_rate)
    }
}
