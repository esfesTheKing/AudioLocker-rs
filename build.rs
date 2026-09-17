use std::error::Error;

extern crate winresource;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        let mut res = winresource::WindowsResource::new();

        res.set_icon_with_id("assets/dark.ico", "dark")
            .set_icon_with_id("assets/light.ico", "light")
            .set_language(0x0009)
            .set_manifest_file("assets/manifest.xml")
            .compile()?;
    }

    Ok(())
}
