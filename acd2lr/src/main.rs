#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate tracing;

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use color_eyre::eyre::Result;
use structopt::StructOpt;

use gio::prelude::*;
use gtk::{prelude::*, Application, ApplicationWindow, Builder};

mod svc;
use svc::*;

mod tr;

mod ui;
use ui::Ui;

#[derive(Debug, StructOpt)]
struct Opts {
    extra_args: Vec<String>,
}

struct App {
    opts: Opts,
}

impl App {
    fn build_ui(&self, app: &Application) {
        // Setup tracing to the statusbar
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        crate::tr::install(tx.clone());

        // Initialize the backend service
        let service = Rc::new(RefCell::new(Some(Service::new(tx).spawn())));

        let glade_src = include_str!("ui/main.glade");
        let builder = Builder::from_string(glade_src);
        let window: ApplicationWindow = builder
            .get_object("main_window")
            .expect("failed to load main window");

        let ui = Ui::new(window.clone(), service.clone(), builder);
        ui.build(rx);

        // Process input arguments
        ui.add_files(
            self.opts
                .extra_args
                .iter()
                .map(|path| PathBuf::from(path))
                .collect(),
        );

        // Set the window parent
        window.set_application(Some(app));

        // Destroy the service on application exit
        window.connect_destroy(move |_| {
            // Take out of the option to terminate the background service
            service.borrow_mut().take();
        });

        info!(ui = true, "Démarrage de acd2lr terminé");
        window.show_all();
    }

    fn run(self) -> Result<()> {
        // Better to let Windows draw title bars than GTK
        if cfg!(target_os = "windows") {
            std::env::set_var("GTK_CSD", "0");
        }

        let application = Application::new(Some("io.github.vtavernier.acd2lr"), Default::default())
            .expect("failed to initialize GTK application");

        application.connect_activate(move |app| self.build_ui(app));

        application.run(&[]);

        Ok(())
    }
}

impl From<Opts> for App {
    fn from(opts: Opts) -> Self {
        Self { opts }
    }
}

#[paw::main]
fn main(opts: Opts) -> Result<()> {
    color_eyre::install()?;

    App::from(opts).run()
}
