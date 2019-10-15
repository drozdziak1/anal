#[macro_use]
extern crate log;

mod anal_notif_handler;
mod constants;

use crossbeam::channel::{self, Receiver, Sender};
use decibel::{AmplitudeRatio, DecibelRatio};
use failure::Error;
use gio::prelude::*;
use gtk::prelude::*;
use jack::{
    AsyncClient, AudioIn, Client, ClientOptions, ClosureProcessHandler, Control,
    NotificationHandler, ProcessHandler,
};
use lazy_static::lazy_static;

use std::{env, marker::Send, sync::Mutex};

use anal_notif_handler::AnalNotifHandler;
use constants::ANAL_JACK_CLIENT_NAME;

pub struct Next;

static MAX_BAR_HEIGHT: f64 = 300.0;
static MIN_GAIN: f64 = -50.0; // How many dbFS we consider silence
static MIN_BAR_X: f64 = 50.0; // How many dbFS we consider silence

fn main() {
    env_logger::init();
    gtk::init().unwrap();
    let uiapp = gtk::Application::new(Some("org.anal.anal"), gio::ApplicationFlags::FLAGS_NONE)
        .expect("Application::new failed");

    let (tx_cmd, rx_cmd) = channel::unbounded();

    let (_jack_client, rx) = prepare_jack_client(rx_cmd).unwrap();

    uiapp.connect_activate(move |app| {
        // We create the main window.
        let win = gtk::ApplicationWindow::new(app);
        let area = gtk::DrawingArea::new();

        // Then we set its size and a title.
        win.set_default_size(500, 500);
        win.set_title("Basic example");
        win.add(&area);

        let rx = rx.clone();
        let tx_cmd = tx_cmd.clone();

        area.connect_draw(move |area_self, ctxt| {
            lazy_static! {
                static ref GAIN: Mutex<f64> = Mutex::new(MIN_GAIN);
            }

            info!("Drawing tick");

            let mut gain_lock = GAIN.lock().unwrap();

            match tx_cmd.send(Next) {
                Ok(()) => info!("GTK: Sent Next to JACK"),
                Err(e) => {
                    error!("Could not ask for next sample: {:?}", e);
                }
            }

            if let Ok(gain) = rx.try_recv() {
                info!(
                    "GTK: Received new gain value {}dbFS (old GAIN: {}dbFS)",
                    gain, *gain_lock
                );
                *gain_lock = gain as f64;
            }

            ctxt.set_source_rgb(0.0, 0.0, 0.0);
            ctxt.paint();
            ctxt.set_source_rgb(1.0, 0.0, 0.0);
            let new_height = if *gain_lock > MIN_GAIN {
                dbg!(*gain_lock / MIN_GAIN);
                (MAX_BAR_HEIGHT - MIN_BAR_X) * (1.0 - (*gain_lock / MIN_GAIN))
            } else {
                0.0
            };

            info!("Applying new height {}", new_height);
            ctxt.rectangle(MIN_BAR_X, 50.0, 5.0, new_height);

            ctxt.fill();
            ctxt.stroke();

            area_self.queue_draw();
            Inhibit(false)
        });

        println!("Showing all");
        // Don't forget to make all widgets visible.
        win.show_all();
    });
    uiapp.run(&env::args().collect::<Vec<_>>());
}

fn prepare_jack_client(
    rx_cmd: Receiver<Next>,
) -> Result<
    (
        AsyncClient<
            impl 'static + Send + Sync + NotificationHandler,
            impl 'static + Send + Sync + ProcessHandler,
        >,
        Receiver<f32>,
    ),
    Error,
> {
    let (client, _status) = Client::new(ANAL_JACK_CLIENT_NAME, ClientOptions::NO_START_SERVER)?;

    let in_1 = client.register_port("in_1", AudioIn::default())?;
    let in_2 = client.register_port("in_2", AudioIn::default())?;

    let (tx, rx) = channel::unbounded();

    let mut_tx = Mutex::new(tx);

    let process_callback = move |_client: &Client, ps: &jack::ProcessScope| -> Control {
        let data_1 = in_1.as_slice(ps);
        let data_2 = in_2.as_slice(ps);

        if let Ok(Next) = rx_cmd.try_recv() {
            let mut sum = 0.0;
            for value in data_1 {
                sum += value;
            }

            let avg = sum / data_1.len() as f32;

            let avg_db: DecibelRatio<_> = AmplitudeRatio(avg).into();
            info!("JACK: Sending gain {:?}dbFS from in_1", avg_db.0);

            (*mut_tx.lock().unwrap()).send(avg_db.0.clone()).unwrap();
        }

        Control::Continue
    };

    let process = ClosureProcessHandler::new(process_callback);

    let active_client = client.activate_async(AnalNotifHandler, process)?;

    Ok((active_client, rx))
}
