//! Bank persistence using only the standard library.
//!
//! Data file: `banks.dat` inside the directory from [`data_directory`].
//!
//! **Render.com:** attach a persistent disk at `/data` (or set `FINSIM_DATA_DIR`
//! to match your mount path). When `RENDER` is `true` and `FINSIM_DATA_DIR` is
//! unset, the default is `/data`. Only paths on the mounted volume persist
//! across deploys.
//!
//! **Local:** unset `FINSIM_DATA_DIR` to use the relative directory `data`.
//!
//! File format: UTF-8 lines, either `# ...` (comments or header) or
//! `id\tescaped_name`.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::models::Bank;

const DEFAULT_DATA_DIR: &str = "data";
/// Used on Render when `RENDER` is set and `FINSIM_DATA_DIR` is not.
const RENDER_DEFAULT_DATA_DIR: &str = "/app/data";
const BANKS_FILE: &str = "banks.dat";
const FILE_HEADER: &str = "# finsim banks v1";

/// Directory that will contain `banks.dat`.
/// Prefer setting `FINSIM_DATA_DIR` on Render to your disk mount path.
pub fn data_directory() -> PathBuf {
    match std::env::var("FINSIM_DATA_DIR") {
        Ok(s) if !s.trim().is_empty() => PathBuf::from(s.trim()),
        _ => {
            if std::env::var("RENDER").ok().as_deref() == Some("true") {
                PathBuf::from(RENDER_DEFAULT_DATA_DIR)
            } else {
                PathBuf::from(DEFAULT_DATA_DIR)
            }
        }
    }
}

pub fn banks_file_path() -> PathBuf {
    data_directory().join(BANKS_FILE)
}

fn escape_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn unescape_name(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                _ => return None,
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn parse_line(line: &str) -> Option<Bank> {
    let line = line.trim_end_matches('\r');
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (id_part, name_part) = line.split_once('\t')?;
    let id: u64 = id_part.parse().ok()?;
    let name = unescape_name(name_part)?;
    Some(Bank::new(id, name))
}

pub fn load_banks(path: &Path) -> Vec<Bank> {
    match try_load_banks(path) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            eprintln!(
                "finsim: warning: could not load banks from {}: {e}",
                path.display()
            );
            Vec::new()
        }
    }
}

fn try_load_banks(path: &Path) -> io::Result<Vec<Bank>> {
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let mut banks = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Some(bank) = parse_line(&line) {
            banks.push(bank);
        }
    }
    Ok(banks)
}

pub fn save_banks(path: &Path, banks: &[Bank]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_name = format!(
        "{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(BANKS_FILE)
    );
    let tmp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(tmp_name);

    {
        let f = File::create(&tmp_path)?;
        let mut w = BufWriter::new(f);
        writeln!(w, "{FILE_HEADER}")?;
        for b in banks {
            writeln!(w, "{}\t{}", b.id, escape_name(&b.name))?;
        }
        w.flush()?;
    }

    fs::rename(&tmp_path, path)?;
    Ok(())
}
