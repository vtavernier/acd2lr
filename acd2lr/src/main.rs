#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate tracing;

use color_eyre::eyre::Result;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use structopt::StructOpt;

use gdk_pixbuf::prelude::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Builder, FileChooserNative, MenuItem, Statusbar};

mod svc;
use svc::*;

mod tr;

#[derive(Debug, StructOpt)]
struct Opts {
    extra_args: Vec<String>,
}

#[derive(Clone)]
struct Ui {
    window: ApplicationWindow,
    service: Rc<RefCell<Option<ServiceHandle>>>,
    builder: Builder,
}

impl Ui {
    fn new(
        window: ApplicationWindow,
        service: Rc<RefCell<Option<ServiceHandle>>>,
        builder: Builder,
    ) -> Self {
        Self {
            window,
            service,
            builder,
        }
    }

    fn open_callback<T>(self, filechooser: FileChooserNative) -> impl for<'r> Fn(&'r T) -> () {
        move |_: &_| {
            filechooser.run();

            let filenames = filechooser.get_filenames();
            self.add_files(filenames);
        }
    }

    fn add_files(&self, filenames: Vec<PathBuf>) {
        if !filenames.is_empty() {
            self.window.set_sensitive(false);

            if let Some(service) = &*self.service.borrow() {
                service.send_request(Request::OpenPaths(filenames));
            }
        }
    }

    fn handle_message(&self, item: Message, statusbar: &Statusbar) {
        match item {
            Message::Status(message) => {
                let context = statusbar.get_context_id("description");
                statusbar.push(context, &message);
            }
            Message::AddPathsComplete(results) => {
                let ok_count = results.iter().filter(|res| res.is_ok()).count();
                let total = results.len();
                let err_count = total - ok_count;

                info!(
                    ui = true,
                    "Fichiers ajoutés: {} ; Erreurs: {}", ok_count, err_count
                );

                let dialog = gtk::MessageDialog::new(
                    Some(&self.window),
                    gtk::DialogFlags::DESTROY_WITH_PARENT | gtk::DialogFlags::MODAL,
                    if total > 0 {
                        if ok_count == 0 {
                            gtk::MessageType::Error
                        } else if err_count == 0 {
                            gtk::MessageType::Info
                        } else {
                            gtk::MessageType::Warning
                        }
                    } else {
                        gtk::MessageType::Warning
                    },
                    gtk::ButtonsType::Ok,
                    &format!("Fichiers ajoutés: {}\nErreurs: {}", ok_count, err_count),
                );

                dialog.connect_response(|dialog, _| {
                    dialog.close();
                });

                dialog.run();

                // Re-enable the window
                self.window.set_sensitive(true);
            }
        }
    }

    fn build(&self, rx: glib::Receiver<Message>) {
        let window = self.window.clone();
        let builder = self.builder.clone();

        // Set window icon
        {
            let icon_loader = gdk_pixbuf::PixbufLoader::new();
            icon_loader.write(include_bytes!("../app.png")).unwrap();
            icon_loader.close().unwrap();

            if let Some(icon) = icon_loader.get_pixbuf() {
                window.set_icon(Some(&icon));
            } else {
                warn!("no icon set");
            }
        }

        let menu_open: MenuItem = builder.get_object("menu_open").unwrap();
        menu_open.connect_activate(
            self.clone()
                .open_callback(builder.get_object("filechooser").unwrap()),
        );

        let menu_open_folder: MenuItem = builder.get_object("menu_open_folder").unwrap();
        menu_open_folder.connect_activate(
            self.clone()
                .open_callback(builder.get_object("filechooser_folder").unwrap()),
        );

        let menu_quit: MenuItem = builder.get_object("menu_quit").unwrap();
        menu_quit.connect_activate({
            let window = window.clone();

            move |_| {
                window.close();
            }
        });

        rx.attach(None, {
            let ui = self.clone();
            let statusbar: Statusbar = builder.get_object("statusbar").unwrap();

            move |item| {
                ui.handle_message(item, &statusbar);
                glib::Continue(true)
            }
        });
    }
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
