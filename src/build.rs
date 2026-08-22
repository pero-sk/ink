use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/command/help.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let help_path = manifest_dir.join("src/command/help.rs");

    let source =
        fs::read_to_string(&help_path).expect("failed to read src/command/help.rs");

    let help = extract_help(&source);

    let man = format!(
        r#".TH INK 1
.SH NAME
ink \- a small terminal-based text editor
.SH SYNOPSIS
.B ink
.RI [ FILE ]
.SH DESCRIPTION
{help}
"#,
        help = escape_roff(&help),
    );

    fs::write(out_dir.join("ink.1"), man)
        .expect("failed to write generated man page");
}

fn extract_help(source: &str) -> String {
    let marker = r##"r#""##;

    let start = source
        .find(marker)
        .expect("HELP must contain a raw string");

    let start = start + marker.len();

    let end = source[start..]
        .find(r##""#"##)
        .expect("HELP raw string is not terminated");

    source[start..start + end].trim().to_string()
}

fn escape_roff(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with('.') || line.starts_with('\'') {
                format!(r"\&{}", line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
