//! The demo's command line: its own flags, then paths.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub const USAGE: &str =
    "usage: shell [--theme ID] [--overrides PATH] [--native] [--all-buttons] [PATH…]

  --theme ID        a bundled theme (default: tokyo-night)
  --overrides PATH  the overrides file to watch (default: $XDG_CONFIG_HOME/strata/overrides.toml);
                    read only, never written
  --native          keep the window manager's frame
  --all-buttons     show Minimize and Maximize too (a fresh Strata profile shows only Close)";

/// What a launch asked for.
#[derive(Debug, Default)]
pub struct Options {
    pub theme: Option<String>,
    pub overrides: Option<PathBuf>,
    pub native: bool,
    pub all_buttons: bool,
    pub help: bool,
    pub paths: Vec<OsString>,
}

impl Options {
    /// The arguments after the program's name. `--` ends the flags.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, String> {
        let mut options = Options::default();
        let mut args = args.into_iter();
        let mut flags = true;
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.to_str() {
                Some("--") if flags => flags = false,
                Some("--theme") if flags => {
                    options.theme = Some(value("--theme")?.to_string_lossy().into_owned());
                }
                Some("--overrides") if flags => {
                    options.overrides = Some(value("--overrides")?.into())
                }
                Some("--native") if flags => options.native = true,
                Some("--all-buttons") if flags => options.all_buttons = true,
                Some("--help" | "-h") if flags => options.help = true,
                Some(flag) if flags && flag.starts_with('-') && flag != "-" => {
                    return Err(format!("unknown option {flag}"));
                }
                _ => options.paths.push(arg),
            }
        }
        Ok(options)
    }

    /// The overrides file: the flag's, else `$XDG_CONFIG_HOME/strata/overrides.toml`
    /// (`~/.config` when unset or not absolute, as the XDG spec says), else none.
    pub fn overrides_path(&self) -> Option<PathBuf> {
        if let Some(path) = &self.overrides {
            return Some(path.clone());
        }
        let absolute = |v: OsString| Some(PathBuf::from(v)).filter(|p| p.is_absolute());
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| {
                std::env::var_os("HOME")
                    .and_then(absolute)
                    .map(|home| home.join(".config"))
            })?;
        Some(config.join("strata").join("overrides.toml"))
    }
}

/// A launch's paths, from its whole command line (the program's name first). A launch whose
/// flags don't parse (another client of the same app id) opens what isn't a flag.
pub fn paths(argv: &[OsString]) -> Vec<OsString> {
    let rest = argv.iter().skip(1).cloned();
    match Options::parse(rest) {
        Ok(options) => options.paths,
        Err(_) => argv
            .iter()
            .skip(1)
            .filter(|a| !a.to_string_lossy().starts_with('-'))
            .cloned()
            .collect(),
    }
}

/// What a window shows for `path`: as given, lossily, so a name that isn't UTF-8 still opens.
pub fn shown(path: &OsStr) -> String {
    path.to_string_lossy().into_owned()
}
