use std::{cell::RefCell, convert::TryInto, ffi::OsString, path::PathBuf, rc::Rc};

use gdk_pixbuf::prelude::*;
use gio::prelude::*;
use glib::clone;
use gtk::{
    prelude::*, ApplicationWindow, Builder, Button, ComboBox, FileChooserNative, ListBox, MenuItem,
    ProgressBar, Statusbar,
};

mod row_data;
use row_data::RowData;

use crate::svc::*;

#[derive(Clone)]
pub struct Ui {
    window: ApplicationWindow,
    service: Rc<RefCell<Option<ServiceHandle>>>,
    builder: Builder,
}

impl Ui {
    pub fn new(
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

    pub fn add_files(&self, filenames: Vec<PathBuf>) {
        if !filenames.is_empty() {
            self.window.set_sensitive(false);

            if let Some(service) = &*self.service.borrow() {
                service.send_request(Request::OpenPaths(filenames));
            }
        }
    }

    fn handle_message(
        &self,
        item: Message,
        statusbar: &Statusbar,
        file_list: &gio::ListStore,
        progress: &ProgressBar,
        controls: &impl gtk::WidgetExt,
    ) {
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
            Message::FileStateUpdate(events) => {
                for event in events {
                    match event {
                        Event::Added { start, files } => {
                            file_list.splice(
                                start as _,
                                0,
                                &files
                                    .into_iter()
                                    .map(RowData::new)
                                    .map(|row_data| row_data.upcast::<glib::Object>())
                                    .collect::<Vec<_>>(),
                            );
                        }
                        Event::Changed { start, files } => {
                            file_list.splice(
                                start as _,
                                files.len() as _,
                                &files
                                    .into_iter()
                                    .map(RowData::new)
                                    .map(|row_data| row_data.upcast::<glib::Object>())
                                    .collect::<Vec<_>>(),
                            );
                        }
                    }
                }
            }
            Message::ProgressUpdate { current, total } => {
                if current == total {
                    progress.set_fraction(0.);
                    controls.set_sensitive(true);
                } else {
                    progress.set_fraction(current as f64 / total as f64);
                    controls.set_sensitive(false);
                }
            }
        }
    }

    pub fn build(&self, rx: glib::Receiver<Message>) {
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
        menu_quit.connect_activate(clone!(@weak window => move |_| {
            window.close();
        }));

        // Create the list model
        let list = gio::ListStore::new(RowData::static_type());
        let listbox: ListBox = builder.get_object("listbox").unwrap();
        listbox.bind_model(Some(&list), move |item| {
            let box_ = gtk::ListBoxRow::new();
            box_.set_margin_start(12);
            box_.set_margin_end(12);

            let item = item.downcast_ref::<RowData>().unwrap();

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);

            let label_path = gtk::Label::new(None);
            item.bind_property("path", &label_path, "label")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
                .build();
            label_path.set_halign(gtk::Align::Start);
            hbox.pack_start(&label_path, true, true, 0);

            let label_state = gtk::Label::new(None);
            item.bind_property("state", &label_state, "label")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
                .build();
            hbox.pack_start(&label_state, false, false, 0);

            box_.add(&hbox);

            box_.show_all();

            box_.upcast::<gtk::Widget>()
        });

        listbox.set_activate_on_single_click(false);
        listbox.connect_row_activated(clone!(@weak list => move |_, row| {
            let file = list.get_object(row.get_index() as _).unwrap();
            let file = file.downcast_ref::<RowData>().unwrap();
            let path = file.path();

            async_std::task::spawn(async move {
                if let Some(p) = async_std::fs::canonicalize(path).await.ok() {
                    tracing::info!(path = %p.display(), "opening");

                    if cfg!(target_os = "linux") {
                        std::process::Command::new("dbus-send").args(&[
                            OsString::from("--session"),
                            OsString::from("--print-reply"),
                            OsString::from("--dest=org.freedesktop.FileManager1"),
                            OsString::from("/org/freedesktop/FileManager1"),
                            OsString::from("org.freedesktop.FileManager1.ShowItems"),
                            {
                                let mut s = OsString::from("array:string:file:");
                                s.push(p);
                                s
                            },
                            OsString::from("string:"),
                        ]).spawn().ok();
                    } else if cfg!(target_os = "windows") {
                        if let Some(explorer) = std::env::var_os("WINDIR").map(|windir| {
                            let mut path = std::path::PathBuf::from(windir);
                            path.push("explorer.exe");
                            path
                        }) {
                            std::process::Command::new(explorer).args(&[
                                {
                                    let mut s = OsString::from("/select,");
                                    s.push(&p);
                                    s
                                }
                            ]).spawn().ok();
                        } else {
                            tracing::warn!("windows folder not found");
                            return;
                        }
                    } else {
                        tracing::warn!("not supported");
                        return;
                    }
                }
            });
        }));

        let button_apply: Button = builder.get_object("button_apply").unwrap();
        let combobox_backups: ComboBox = builder.get_object("combobox_backups").unwrap();
        button_apply.connect_clicked({
            let svc = self.service.clone();

            move |_| {
                if let Some(service) = &*svc.borrow() {
                    service.send_request(Request::Apply(
                        combobox_backups
                            .get_active()
                            .unwrap_or(0)
                            .try_into()
                            .unwrap(),
                    ));
                }
            }
        });

        rx.attach(None, {
            let ui = self.clone();
            let statusbar: Statusbar = builder.get_object("statusbar").unwrap();
            let progress: ProgressBar = builder.get_object("progressbar").unwrap();
            let box_: gtk::Box = builder.get_object("box_controls").unwrap();

            move |item| {
                ui.handle_message(item, &statusbar, &list, &progress, &box_);
                glib::Continue(true)
            }
        });
    }
}
