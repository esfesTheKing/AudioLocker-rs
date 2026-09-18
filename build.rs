use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        let mut res = winresource::WindowsResource::new();

        res.set_icon_with_id("assets/dark.ico", "dark")
            .set_icon_with_id("assets/light.ico", "light")
            .set_language(0x0009)
            .set_manifest_file("assets/manifest.xml")
            .compile()?;

        let mut bindings_rs_file = File::create("src/bindings.rs")?;

        for bindgen_filename in ["audio", "com", "shell"] {
            let src_bindgen_filename = format!("bindings/{bindgen_filename}.txt");
            let dst_bindgen_filename = format!("src/bindings/{bindgen_filename}.rs");

            println!("cargo:rerun-if-changed={src_bindgen_filename}");
            println!("cargo:rerun-if-changed={dst_bindgen_filename}");
            windows_bindgen::bindgen(["--out", &dst_bindgen_filename, "--flat", "--etc", &src_bindgen_filename]);

            writeln!(bindings_rs_file, "#[allow(warnings)]")?;
            writeln!(bindings_rs_file, "pub mod {bindgen_filename};")?;
            writeln!(bindings_rs_file, "")?;
        }
    }

    Ok(())
}
