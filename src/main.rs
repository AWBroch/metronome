use std::convert::identity;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use fruitbasket::Trampoline;
use iced::widget::{column, container, row, slider, text};
use iced::{executor, time, Application, Command, Element, Settings, Theme};
use iced_native::widget::{button, checkbox, vertical_space};
use iced_native::{color, Length};
use lazy_static::lazy_static;
use rodio::{
    source::{Buffered, SamplesConverter},
    Decoder, OutputStream, Source,
};
use widgets::circle;

mod widgets;

const E_CLICK: &'static [u8] = include_bytes!("../assets/e-click.wav");
const E_FLAT_CLICK: &'static [u8] = include_bytes!("../assets/e-flat-click.wav");
const F_CLICK: &'static [u8] = include_bytes!("../assets/f-click.wav");

static OFF_BEAT: AtomicBool = AtomicBool::new(true);

lazy_static! {
    static ref E_CLICK_SOURCE: Buffered<SamplesConverter<Decoder<Cursor<&'static [u8]>>, f32>> =
        Decoder::new(Cursor::new(E_CLICK))
            .unwrap()
            .convert_samples()
            .buffered();
    static ref E_FLAT_CLICK_SOURCE: Buffered<SamplesConverter<Decoder<Cursor<&'static [u8]>>, f32>> =
        Decoder::new(Cursor::new(E_FLAT_CLICK))
            .unwrap()
            .convert_samples()
            .buffered();
    static ref F_CLICK_SOURCE: Buffered<SamplesConverter<Decoder<Cursor<&'static [u8]>>, f32>> =
        Decoder::new(Cursor::new(F_CLICK))
            .unwrap()
            .convert_samples()
            .buffered();
}

fn main() {
    Trampoline::new("Metronome", "metronome", "com.brochweb.metronome")
        .icon("Metronome")
        .version(env!("CARGO_PKG_VERSION"))
        .resource("assets/Metronome.icns")
        .build(fruitbasket::InstallDir::Custom(String::from("build")))
        .unwrap();

    Metronome::run(Settings::default()).unwrap();
}

struct Metronome {
    bar: u32,
    bpm: u32,
    state: MetroState,
    accentuate_first_beat: bool,
    off_beats: bool,
    player_thread: Sender<Beat>,
    vol_tx: Sender<f32>,
    volume: f32,
}

struct MetronomeSettings {
    bar: u32,
    bpm: u32,
    accentuate_first_beat: bool,
    off_beats: bool,
    volume: f32,
}

impl Default for MetronomeSettings {
    fn default() -> Self {
        Self {
            bar: 4,
            bpm: 100,
            accentuate_first_beat: true,
            off_beats: false,
            volume: 1.0,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
enum MetroState {
    Stopped,
    FirstBeat,
    Beat(u32),
}

#[derive(Debug, Clone)]
enum Message {
    Toggle,
    Beat,
    OffBeat,
    BPMUpdate(u32),
    BarUpdate(u32),
    FirstBeats(bool),
    OffBeats(bool),
    SetVolume(f32),
}

impl Application for Metronome {
    type Executor = executor::Default;
    type Flags = MetronomeSettings;
    type Message = Message;
    type Theme = Theme;

    fn new(flags: MetronomeSettings) -> (Metronome, Command<Self::Message>) {
        let (tx, rx) = mpsc::channel();
        let (vol_tx, vol_rx) = mpsc::channel();
        std::thread::spawn(move || player_thread(rx, flags.volume, vol_rx));
        (
            Metronome {
                state: MetroState::Stopped,
                bar: flags.bar,
                bpm: flags.bpm,
                accentuate_first_beat: flags.accentuate_first_beat,
                off_beats: flags.off_beats,
                player_thread: tx,
                volume: flags.volume,
                vol_tx,
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match self.state {
            MetroState::Beat(_) | MetroState::FirstBeat => {
                if self.off_beats {
                    time::every(Duration::from_secs_f64(60. / self.bpm as f64 / 2.)).map(|_| {
                        if OFF_BEAT
                            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
                            .is_ok()
                        {
                            Message::OffBeat
                        } else {
                            OFF_BEAT.store(true, Ordering::Relaxed);
                            Message::Beat
                        }
                    })
                } else {
                    time::every(Duration::from_secs_f64(60. / self.bpm as f64))
                        .map(|_| Message::Beat)
                }
            }
            MetroState::Stopped => iced::Subscription::none(),
        }
    }

    fn title(&self) -> String {
        String::from("Metronome")
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Toggle => {
                if self.state == MetroState::Stopped {
                    self.state = MetroState::Beat(self.bar - 1);
                    Command::perform(async {}, |()| Message::Beat)
                } else {
                    self.state = MetroState::Stopped;
                    Command::none()
                }
            }
            Message::BPMUpdate(bpm) => {
                self.bpm = bpm;
                Command::none()
            }
            Message::BarUpdate(bar) => {
                self.bar = bar;
                Command::none()
            }
            Message::FirstBeats(val) => {
                self.accentuate_first_beat = val;
                Command::none()
            }
            Message::OffBeats(val) => {
                self.off_beats = val;
                Command::none()
            }
            Message::SetVolume(vol) => {
                self.volume = vol;
                self.vol_tx.send(vol).unwrap();
                Command::none()
            }
            Message::Beat => {
                match self.state {
                    MetroState::FirstBeat => {
                        self.player_thread.send(Beat::Beat).unwrap();
                        self.state = MetroState::Beat(1);
                    }
                    MetroState::Beat(beat) => {
                        self.player_thread
                            .send(if self.accentuate_first_beat && beat >= self.bar - 1 {
                                Beat::FirstBeat
                            } else {
                                Beat::Beat
                            })
                            .unwrap();
                        if beat >= self.bar - 1 {
                            self.state = MetroState::FirstBeat;
                        } else {
                            self.state = MetroState::Beat(beat + 1);
                        }
                    }
                    MetroState::Stopped => unreachable!(),
                };
                Command::none()
            }
            Message::OffBeat => {
                self.player_thread.send(Beat::OffBeat).unwrap();
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        let mut beats = Vec::new();
        let current_beat = match self.state {
            MetroState::Beat(n) => Some(n),
            MetroState::FirstBeat => Some(0),
            _ => None,
        };
        for i in 0..self.bar {
            beats.push(
                circle(
                    25.0,
                    if Some(i) == current_beat {
                        color!(0x6080df)
                    } else {
                        color!(0xe0e0e0)
                    },
                )
                .into(),
            )
        }
        container(
            column![
                vertical_space(25.0),
                text("Metronome")
                    .size(72)
                    .horizontal_alignment(iced_native::alignment::Horizontal::Center),
                container(
                    column![
                        column![
                            text(format!("{} BPM", self.bpm)).size(46),
                            slider(30..=300, self.bpm, |v| Message::BPMUpdate(v)),
                            row(beats).spacing(5.0),
                        ]
                        .spacing(30.0)
                        .align_items(iced_native::Alignment::Center),
                        column![
                            text(format!("{} beats per bar", self.bar)),
                            slider(2..=16, self.bar, |v| Message::BarUpdate(v)),
                            row![
                                checkbox("First beat accent", self.accentuate_first_beat, |val| {
                                    Message::FirstBeats(val)
                                })
                                .width(Length::FillPortion(1)),
                                checkbox("Off-beats", self.off_beats, |val| Message::OffBeats(val))
                                    .width(Length::FillPortion(1))
                            ]
                            .align_items(iced_native::Alignment::Center)
                            .width(450),
                            "Volume:",
                            row![
                                slider(0.1..=5.0, self.volume, |val| Message::SetVolume(val))
                                    .step(0.01),
                                text(format!("{}%", (self.volume * 100.).round()))
                            ]
                            .spacing(5.0)
                        ]
                        .align_items(iced_native::Alignment::Center)
                        .spacing(10.0),
                        button(
                            text(if self.state == MetroState::Stopped {
                                "Start"
                            } else {
                                "Stop"
                            })
                            .size(32)
                            .horizontal_alignment(iced_native::alignment::Horizontal::Center)
                        )
                        .width(150.0)
                        .on_press(Message::Toggle)
                    ]
                    .spacing(30.0)
                    .align_items(iced_native::Alignment::Center)
                    .max_width(450)
                )
                .height(Length::Fill)
                .center_y()
                .center_x(),
            ]
            .align_items(iced_native::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .into()
    }
}

enum Beat {
    FirstBeat,
    OffBeat,
    Beat,
}

fn player_thread(rx: Receiver<Beat>, volume: f32, vol_rx: Receiver<f32>) {
    let mut volume = volume;
    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    while let Ok(beat) = rx.recv() {
        while let Ok(new_vol) = vol_rx.try_recv() {
            volume = new_vol;
        }
        stream_handle
            .play_raw(
                match beat {
                    Beat::Beat => E_CLICK_SOURCE.clone(),
                    Beat::FirstBeat => E_FLAT_CLICK_SOURCE.clone(),
                    Beat::OffBeat => F_CLICK_SOURCE.clone(),
                }
                .amplify(volume),
            )
            .unwrap();
    }
    identity(stream);
}
