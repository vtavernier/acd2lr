use std::path::PathBuf;
use std::sync::Arc;

use glib::subclass;
use glib::subclass::prelude::*;
use glib::translate::*;
use glib::{glib_object_impl, glib_object_subclass, glib_wrapper};
use glib::{Cast, GBoxed, ObjectExt, StaticType, ToValue};

use crate::svc::MetadataFile;

#[derive(Clone, GBoxed)]
#[gboxed(type_name = "ArcFile")]
struct ArcFile(Arc<MetadataFile>);

impl std::ops::Deref for ArcFile {
    type Target = MetadataFile;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

// Implementation sub-module of the GObject
mod imp {
    use super::*;
    use std::cell::RefCell;

    // The actual data structure that stores our values. This is not accessible
    // directly from the outside.
    pub struct RowData {
        inner: RefCell<Option<ArcFile>>,
    }

    // GObject property definitions for our two values
    static PROPERTIES: [subclass::Property; 3] = [
        subclass::Property("path", |path| {
            glib::ParamSpec::string(
                path,
                "Path",
                "Path to the target file",
                None, // Default value
                glib::ParamFlags::READABLE,
            )
        }),
        subclass::Property("state", |state| {
            glib::ParamSpec::string(
                state,
                "State",
                "File processing state",
                None, // Default value
                glib::ParamFlags::READABLE,
            )
        }),
        subclass::Property("inner", |inner| {
            glib::ParamSpec::boxed(
                inner,
                "Inner",
                "Inner file structure",
                ArcFile::get_type(),
                glib::ParamFlags::READWRITE,
            )
        }),
    ];

    // Basic declaration of our type for the GObject type system
    impl ObjectSubclass for RowData {
        const NAME: &'static str = "RowData";
        type ParentType = glib::Object;
        type Instance = subclass::simple::InstanceStruct<Self>;
        type Class = subclass::simple::ClassStruct<Self>;

        glib_object_subclass!();

        // Called exactly once before the first instantiation of an instance. This
        // sets up any type-specific things, in this specific case it installs the
        // properties so that GObject knows about their existence and they can be
        // used on instances of our type
        fn class_init(klass: &mut Self::Class) {
            klass.install_properties(&PROPERTIES);
        }

        // Called once at the very beginning of instantiation of each instance and
        // creates the data structure that contains all our state
        fn new() -> Self {
            Self {
                inner: RefCell::new(None),
            }
        }
    }

    // The ObjectImpl trait provides the setters/getters for GObject properties.
    // Here we need to provide the values that are internally stored back to the
    // caller, or store whatever new value the caller is providing.
    //
    // This maps between the GObject properties and our internal storage of the
    // corresponding values of the properties.
    impl ObjectImpl for RowData {
        glib_object_impl!();

        fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
            let prop = &PROPERTIES[id];

            match *prop {
                subclass::Property("inner", ..) => {
                    if let Ok(val) = value.get_some::<&ArcFile>() {
                        *self.inner.borrow_mut() = Some(val.clone());
                    }
                }
                _ => {}
            }
        }

        fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
            let prop = &PROPERTIES[id];

            if let Some(inner) = self.inner.borrow().as_ref() {
                match *prop {
                    subclass::Property("inner", ..) => Ok(inner.clone().to_value()),
                    subclass::Property("path", ..) => {
                        Ok(inner.path().display().to_string().to_value())
                    }
                    subclass::Property("state", ..) => Ok(inner.state().to_string().to_value()),
                    _ => Err(()),
                }
            } else {
                Err(())
            }
        }
    }
}

// Public part of the RowData type. This behaves like a normal gtk-rs-style GObject
// binding
glib_wrapper! {
    pub struct RowData(Object<subclass::simple::InstanceStruct<imp::RowData>, subclass::simple::ClassStruct<imp::RowData>, RowDataClass>);

    match fn {
        get_type => || imp::RowData::get_type().to_glib(),
    }
}

// Constructor for new instances. This simply calls glib::Object::new() with
// initial values for our two properties and then returns the new instance
impl RowData {
    pub fn new(inner: Arc<MetadataFile>) -> RowData {
        glib::Object::new(Self::static_type(), &[("inner", &ArcFile(inner))])
            .expect("Failed to create row data")
            .downcast()
            .expect("Created row data is of wrong type")
    }

    pub fn inner(&self) -> Arc<MetadataFile> {
        // TODO: Don't clone and borrow instead
        self.get_property("inner")
            .unwrap()
            .get_some::<&ArcFile>()
            .unwrap()
            .0
            .clone()
    }

    pub fn path(&self) -> PathBuf {
        self.inner().path().to_path_buf()
    }
}
