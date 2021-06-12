#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate tracing;

use color_eyre::eyre::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opts {
    extra_args: Vec<String>,
}

struct App {
    opts: Opts,
}

type MessageSender = glib::Sender<String>;

impl App {
    fn run(self) -> Result<()> {
        use gio::prelude::*;
        use gtk::prelude::*;
        use gdk_pixbuf::prelude::*;

        use gtk::{Application, ApplicationWindow, MenuItem, Statusbar};

        // Better to let Windows draw title bars than GTK
        if cfg!(target_os = "windows") {
            std::env::set_var("GTK_CSD", "0");
        }

        let application = Application::new(Some("io.github.vtavernier.acd2lr"), Default::default())
            .expect("failed to initialize GTK application");

        application.connect_activate({
            move |app| {
                // Setup tracing to the statusbar
                let (tx, rx) = glib::MainContext::channel::<String>(glib::PRIORITY_DEFAULT);
                install_tracing(tx);

                let glade_src = include_str!("ui/main.glade");
                let builder = gtk::Builder::from_string(glade_src);
                let window: ApplicationWindow = builder
                    .get_object("main_window")
                    .expect("failed to load main window");

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


                let filechooser: gtk::FileChooserNative =
                    builder.get_object("filechooser").unwrap();

                let filechooser_folder: gtk::FileChooserNative =
                    builder.get_object("filechooser_folder").unwrap();

                let menu_open: MenuItem = builder.get_object("menu_open").unwrap();
                menu_open.connect_activate({
                    move |_| {
                        filechooser.run();
                    }
                });

                let menu_open_folder: MenuItem = builder.get_object("menu_open_folder").unwrap();
                menu_open_folder.connect_activate({
                    move |_| {
                        filechooser_folder.run();
                    }
                });

                let menu_quit: MenuItem = builder.get_object("menu_quit").unwrap();
                menu_quit.connect_activate({
                    let window = window.clone();
                    move |_| {
                        window.close();
                    }
                });

                let statusbar: Statusbar = builder.get_object("statusbar").unwrap();
                rx.attach(None, {
                    move |item| {
                        let context = statusbar.get_context_id("description");
                        statusbar.push(context, &item);
                        glib::Continue(true)
                    }
                });

                window.set_application(Some(app));

                info!(ui_message = "acd2lr started");
                window.show_all();
            }
        });

        application.run(&self.opts.extra_args);

        Ok(())
    }
}

impl From<Opts> for App {
    fn from(opts: Opts) -> Self {
        Self { opts }
    }
}

fn install_tracing(tx: MessageSender) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ENTER);

    struct ForwardLayer {
        tx: MessageSender,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ForwardLayer {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            struct V<'s> {
                this: &'s ForwardLayer,
            }

            impl<'s> tracing::field::Visit for V<'s> {
                fn record_debug(
                    &mut self,
                    _field: &tracing::field::Field,
                    _value: &dyn std::fmt::Debug,
                ) {
                }

                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    if field.name() == "ui_message" {
                        self.this.tx.send(value.to_owned()).unwrap();
                    }
                }
            }

            event.record(&mut V { this: self });
        }
    }

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ForwardLayer { tx })
        .with(ErrorLayer::default())
        .init();
}

#[paw::main]
fn main(opts: Opts) -> Result<()> {
    color_eyre::install()?;

    App::from(opts).run()
}
