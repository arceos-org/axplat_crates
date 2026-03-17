use std::env;
use std::fs::File;
use std::io::{self, Write};

use camino::{Utf8Path, Utf8PathBuf};

const TEMPLATE_DIR: &str = "template";

fn get_files_recursively(dir: &Utf8Path, prefix: &str) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = Utf8PathBuf::from_path_buf(entry?.path()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "template directory contains a non-UTF-8 path",
            )
        })?;
        let path_name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to get template file name",
            )
        })?;
        let full_name = if prefix.is_empty() {
            path_name.to_owned()
        } else {
            format!("{prefix}/{path_name}")
        };
        if path.is_dir() {
            files.extend(get_files_recursively(&path, &full_name)?);
        } else {
            files.push(full_name);
        }
    }
    Ok(files)
}

fn main() -> io::Result<()> {
    let src_dir = Utf8PathBuf::from(
        env::var("CARGO_MANIFEST_DIR")
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?,
    )
    .join(TEMPLATE_DIR);
    let out_path = Utf8PathBuf::from(
        env::var("OUT_DIR").map_err(|e| io::Error::new(io::ErrorKind::NotFound, e.to_string()))?,
    )
    .join("template.rs");

    let mut f = File::create(out_path.as_std_path())?;
    writeln!(f, "pub const TEMPLATE: &[(&str, &str)] = &[")?;

    let files = get_files_recursively(&src_dir, "")?;
    for file in files {
        if file == "_Cargo.toml" {
            // `Cargo.toml` is not allowed to be included as a template file in
            // a cargo package , use `_Cargo.toml` instead
            let include_path = src_dir.join("_Cargo.toml");
            writeln!(
                f,
                "    (\"Cargo.toml\", include_str!(r#\"{}\"#)),",
                include_path,
            )?;
        } else {
            let include_path = src_dir.join(&file);
            writeln!(
                f,
                "    (\"{file}\", include_str!(r#\"{}\"#)),",
                include_path,
            )?;
        }
    }
    writeln!(f, "];")?;
    Ok(())
}
