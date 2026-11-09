//! The demo's stand-in for Strata's file side: entry ids by path, listings read from disk,
//! the volume lookup and the transfers (logged; with `--apply`, done, inside `--root` only).

use std::collections::HashMap;
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

/// Copies or moves `sources` into `dest`, each under a name not taken. Refuses anything
/// outside `root`. The names written, or why nothing was.
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
    if !dest.starts_with(&root) {
        return Err(format!("{} is outside the root", dest.display()));
    }
    let mut written = Vec::new();
    for src in sources {
        let src = src.canonicalize().map_err(|e| format!("source: {e}"))?;
        if !src.starts_with(&root) || src == root {
            return Err(format!("{} is outside the root", src.display()));
        }
        if dest.starts_with(&src) || src.parent() == Some(dest.as_path()) {
            continue;
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let to = free_name(&dest, &name);
        if moving {
            if std::fs::rename(&src, &to).is_err() {
                copy_all(&src, &to).map_err(|e| e.to_string())?;
                remove_all(&src).map_err(|e| e.to_string())?;
            }
        } else {
            copy_all(&src, &to).map_err(|e| e.to_string())?;
        }
        written.push(
            to.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    Ok(written)
}

fn free_name(dest: &Path, name: &str) -> PathBuf {
    let first = dest.join(name);
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| dest.join(format!("{name} ({n})")))
        .find(|p| !p.exists())
        .expect("a free name")
}

fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_all(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

fn remove_all(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}
