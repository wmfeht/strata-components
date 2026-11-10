//! The demo's stand-in for Strata's file side: entry ids by path, listings read from disk,
//! the volume lookup and the transfers (logged; with `--apply`, done, inside `--root` only).

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use strata_components::contract::dnd::{VolumeRelation, file_uri, uri_to_path};
use strata_components::contract::{EntryId, EntryKind, ListEntry, ListSource};

/// Entry ids by path, for one window: an id lives for the process and is never reused (W1).
#[derive(Default)]
pub struct Ids {
    by_path: HashMap<PathBuf, EntryId>,
    by_id: HashMap<EntryId, PathBuf>,
    next: u64,
}

impl Ids {
    pub fn of(&mut self, path: &Path) -> EntryId {
        if let Some(id) = self.by_path.get(path) {
            return *id;
        }
        self.next += 1;
        let id = EntryId::new(self.next);
        self.by_path.insert(path.to_owned(), id);
        self.by_id.insert(id, path.to_owned());
        id
    }

    pub fn path(&self, id: EntryId) -> Option<&Path> {
        self.by_id.get(&id).map(PathBuf::as_path)
    }

    pub fn uri(&self, id: EntryId) -> Option<String> {
        self.path(id).map(file_uri)
    }
}

/// `folder`'s entries, folders first then by name (case folded), hidden ones left out.
pub fn list(ids: &mut Ids, folder: &Path) -> ListSource {
    let mut entries: Vec<(bool, String, PathBuf)> = std::fs::read_dir(folder)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        return None;
                    }
                    let dir = e.file_type().is_ok_and(|t| t.is_dir());
                    Some((dir, name, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    ListSource::new(
        entries
            .into_iter()
            .map(|(dir, name, path)| {
                let (kind, icon) = if dir {
                    (EntryKind::Folder, "folder")
                } else {
                    (EntryKind::File, "file")
                };
                ListEntry::new(ids.of(&path), name, kind, icon)
            })
            .collect(),
    )
}

/// Paths from a drop's URIs, local files only.
pub fn paths(uris: &[String]) -> Vec<PathBuf> {
    uris.iter().filter_map(|u| uri_to_path(u)).collect()
}

/// The demo's volumes: everything under `<root>/usb` is another device.
pub fn volume(root: &Path, sources: &[PathBuf], dest: &Path) -> VolumeRelation {
    if sources.is_empty() {
        return VolumeRelation::Unknown;
    }
    let usb = root.join("usb");
    let on_usb = |p: &Path| p.starts_with(&usb);
    if sources.iter().all(|s| on_usb(s) == on_usb(dest)) {
        VolumeRelation::Same
    } else {
        VolumeRelation::Different
    }
}

/// Whether a drop of `sources` into `dest` would do nothing: an item into its own folder, a
/// folder into itself or into one of its descendants (Strata's guard).
pub fn no_op(sources: &[PathBuf], dest: &Path) -> bool {
    !sources.is_empty()
        && sources
            .iter()
            .all(|s| s.parent() == Some(dest) || dest.starts_with(s))
}

/// Copies or moves `sources` into `dest`, each under a name not taken. Refuses the whole
/// drop if any source or the destination is outside `root`, before touching anything. A link
/// is moved or copied as the link (never followed out of the root), and nothing is created
/// over an existing entry (a dangling link included). The names written, or why nothing was.
pub fn apply(
    root: &Path,
    sources: &[PathBuf],
    dest: &Path,
    moving: bool,
) -> Result<Vec<String>, String> {
    let root = root.canonicalize().map_err(|e| format!("root: {e}"))?;
    let dest = dest
        .canonicalize()
        .map_err(|e| format!("destination: {e}"))?;
    if !dest.starts_with(&root) || !dest.is_dir() {
        return Err(format!("{} is no folder inside the root", dest.display()));
    }
    let sources = sources
        .iter()
        .map(|s| inside(&root, s))
        .collect::<Result<Vec<_>, _>>()?;
    let mut written = Vec::new();
    for src in sources {
        if dest.starts_with(&src) || src.parent() == Some(dest.as_path()) {
            continue;
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let to = place(&dest, &name, |to| {
            if moving {
                move_all(&src, to)
            } else {
                copy_all(&src, to)
            }
        })
        .map_err(|e| format!("{}: {e}", src.display()))?;
        written.push(
            to.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    Ok(written)
}

/// `src` with its folder resolved but not itself (a dragged link is the link), if it is there
/// and inside `root`.
fn inside(root: &Path, src: &Path) -> Result<PathBuf, String> {
    let outside = || format!("{} is outside the root", src.display());
    let name = src.file_name().ok_or_else(outside)?;
    let folder = src
        .parent()
        .ok_or_else(outside)?
        .canonicalize()
        .map_err(|e| format!("source: {e}"))?;
    let src = folder.join(name);
    src.symlink_metadata().map_err(|e| format!("source: {e}"))?;
    if !src.starts_with(root) || src == root {
        return Err(outside());
    }
    Ok(src)
}

/// Makes an entry in `dest` with `make`, under `name` or the first "name (n)" free: `make`
/// creates it exclusively, failing with `AlreadyExists` only when the name is taken.
fn place(
    dest: &Path,
    name: &str,
    mut make: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    for n in 0u32.. {
        let to = match n {
            0 => dest.join(name),
            n => dest.join(format!("{name} ({n})")),
        };
        match make(&to) {
            Ok(()) => return Ok(to),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("no free name"))
}

/// An error below the entry `place` made: never "the name is taken", which would have it
/// try another name over a half-made entry.
fn below(e: io::Error) -> io::Error {
    if e.kind() == io::ErrorKind::AlreadyExists {
        io::Error::other(e)
    } else {
        e
    }
}

/// Copies `from` to `to`, which must not exist: a link as the link, a folder with its entries.
fn copy_all(from: &Path, to: &Path) -> io::Result<()> {
    let meta = from.symlink_metadata()?;
    let kind = meta.file_type();
    if kind.is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(from)?, to)
    } else if kind.is_dir() {
        std::fs::create_dir(to)?;
        for e in std::fs::read_dir(from).map_err(below)? {
            let e = e.map_err(below)?;
            copy_all(&e.path(), &to.join(e.file_name())).map_err(below)?;
        }
        std::fs::set_permissions(to, meta.permissions()).map_err(below)
    } else if kind.is_file() {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)?;
        let mut file = std::fs::File::open(from).map_err(below)?;
        io::copy(&mut file, &mut out).map_err(below)?;
        out.set_permissions(meta.permissions()).map_err(below)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "not a file, folder or link",
        ))
    }
}

/// Moves `from` to `to`, which must not exist, never replacing an entry there.
fn move_all(from: &Path, to: &Path) -> io::Result<()> {
    if from.symlink_metadata()?.file_type().is_dir() {
        // Reserve the name with an empty folder of our own, which `rename` replaces (it only
        // replaces an empty folder); across devices, copy and remove instead.
        std::fs::create_dir(to)?;
        if std::fs::rename(from, to).is_ok() {
            return Ok(());
        }
        std::fs::remove_dir(to).map_err(below)?;
        copy_all(from, to)?;
        return std::fs::remove_dir_all(from).map_err(below);
    }
    // A hard link fails if `to` exists (and links a link itself, not what it points at).
    match std::fs::hard_link(from, to) {
        Ok(()) => std::fs::remove_file(from).map_err(below),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => {
            copy_all(from, to)?;
            std::fs::remove_file(from).map_err(below)
        }
    }
}

#[cfg(test)]
mod tests;
