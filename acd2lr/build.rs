use serde::Serialize;
use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct Context {
    version_major: String,
    version_minor: String,
    version_patch: String,
}

fn main() -> anyhow::Result<()> {
    let mut out_res_path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    out_res_path.push("acd2lr.rc");

    let version_major = std::env::var("CARGO_PKG_VERSION_MAJOR")?;
    let version_minor = std::env::var("CARGO_PKG_VERSION_MINOR")?;
    let version_patch = std::env::var("CARGO_PKG_VERSION_PATCH")?;

    let src = std::fs::read_to_string("acd2lr.rc")?;

    let context = Context {
        version_major,
        version_minor,
        version_patch,
    };

    let mut tt = TinyTemplate::new();
    tt.add_template("rc", &src)?;

    std::fs::write(&out_res_path, tt.render("rc", &context)?)?;

    // Embed resources
    embed_resource::compile(&out_res_path);

    Ok(())
}
