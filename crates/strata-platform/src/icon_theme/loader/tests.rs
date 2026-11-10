use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{IconLoader, IconState, Options};
use crate::icon_theme::request::IconRequest;
use crate::icon_theme::resolver::SearchPath;
use crate::icon_theme::test_support::{Scratch, fixtures};

const WAIT: Duration = Duration::from_secs(10);

/// A loader over `roots`, starting in `theme`, and how often its waker has been called.
fn loader(roots: Vec<std::path::PathBuf>, theme: &str) -> (IconLoader, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let options = Options {
        search_path: SearchPath {
            icon_dirs: roots,
            pixmap_dirs: Vec::new(),
        },
        theme: theme.into(),
        ..Options::default()
    };
    let waker: rune::Waker = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let loader = IconLoader::start(waker, options).unwrap_or_else(|e| panic!("start: {e}"));
    (loader, wakes)
}

fn decode_loader() -> (IconLoader, Arc<AtomicUsize>) {
    loader(vec![fixtures().join("decode")], "Decode")
}

/// Waits until the waker has been called `n` times. The lookup calls it last, after the
/// state `poll` reads; a found icon's decode calls it once more, in either order.
fn woken(wakes: &AtomicUsize, n: usize) -> bool {
    let deadline = Instant::now() + WAIT;
    while wakes.load(Ordering::SeqCst) < n {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::yield_now();
    }
    true
}

/// Waits for `poll` to turn true, as an app's `poll` would see it.
fn polled(loader: &IconLoader) -> bool {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if loader.poll() {
            return true;
        }
        std::thread::yield_now();
    }
    false
}

#[test]
fn a_found_svg_wakes_the_app_and_decodes_at_size_times_scale() {
    let (icons, wakes) = decode_loader();
    let icon = icons.load(&IconRequest::named(["good"], 22, 2.0));
    assert_eq!(icon.wait(WAIT), IconState::Ready);
    assert!(
        woken(&wakes, 2),
        "the lookup and the decode each wake the app"
    );
    assert!(icons.poll());
    assert_eq!(icon.image().and_then(rune::Image::size), Some([44, 44]));
    assert!(icon.pixels().is_some());
    assert!(!icon.is_symbolic());
    assert!(
        icon.path()
            .is_some_and(|p| p.ends_with("Decode/scalable/apps/good.svg"))
    );
    // Nothing settled since.
    assert!(!icons.poll());
}

#[test]
fn a_png_decodes_at_its_own_size() {
    let (icons, _) = decode_loader();
    let icon = icons.load(&IconRequest::named(["good-png"], 48, 1.0));
    assert_eq!(icon.wait(WAIT), IconState::Ready);
    assert_eq!(icon.image().and_then(rune::Image::size), Some([16, 16]));
}

#[test]
fn a_missing_icon_settles_as_not_found_and_wakes() {
    let (icons, wakes) = decode_loader();
    let icon = icons.load(&IconRequest::named(
        ["nothing", "nothing-symbolic"],
        22,
        1.0,
    ));
    assert_eq!(icon.wait(WAIT), IconState::NotFound);
    assert!(woken(&wakes, 1));
    assert!(icons.poll());
    assert!(!icons.poll());
    assert!(icon.pixels().is_none() && icon.path().is_none());
}

#[test]
fn a_broken_file_fails_without_trying_another() {
    let (icons, _) = decode_loader();
    for name in ["bad", "bad-svg"] {
        let icon = icons.load(&IconRequest::named([name, "good"], 22, 1.0));
        assert!(
            matches!(icon.wait(WAIT), IconState::Failed(_)),
            "{name}: {icon:?}"
        );
    }
    let missing = icons.load(&IconRequest::file(
        fixtures().join("decode/none.png"),
        22,
        1.0,
    ));
    assert!(matches!(missing.wait(WAIT), IconState::Failed(_)));
}

#[test]
fn a_file_over_the_limit_is_refused() {
    let wakes: rune::Waker = Arc::new(|| {});
    let options = Options {
        max_file_bytes: 8,
        search_path: SearchPath::default(),
        ..Options::default()
    };
    let icons = IconLoader::start(wakes, options).unwrap_or_else(|e| panic!("start: {e}"));
    let path = fixtures().join("decode/Decode/scalable/apps/good.svg");
    let icon = icons.load(&IconRequest::file(path, 22, 1.0));
    assert!(matches!(icon.wait(WAIT), IconState::Failed(_)));
    assert!(icon.image().is_none());
}

#[test]
fn a_symbolic_icon_says_so() {
    let (icons, _) = decode_loader();
    let icon = icons.load(&IconRequest::named(["good-symbolic", "good"], 16, 1.0));
    assert_eq!(icon.wait(WAIT), IconState::Ready);
    assert!(icon.is_symbolic());
}

#[test]
fn the_same_request_shares_one_handle_and_recent_ones_stay() {
    let (icons, _) = decode_loader();
    let request = IconRequest::named(["good"], 22, 1.0);
    let first = icons.load(&request);
    first.wait(WAIT);
    let again = icons.load(&request);
    assert!(Arc::ptr_eq(&first.0, &again.0));
    drop((first, again));
    // No handle left: the recent icons kept it.
    let kept = icons.load(&request);
    assert_eq!(kept.state(), IconState::Ready);
    // Another size or scale is another icon.
    let other = icons.load(&IconRequest::named(["good"], 22, 2.0));
    assert!(!Arc::ptr_eq(&kept.0, &other.0));
}

#[test]
fn an_icon_too_large_for_the_recent_ones_goes_when_its_handles_do() {
    let (icons, _) = decode_loader();
    let request = IconRequest::named(["good"], 200, 2.0);
    let first = icons.load(&request);
    first.wait(WAIT);
    let weak = Arc::downgrade(&first.0);
    drop(first);
    assert!(weak.upgrade().is_none());
}

#[test]
fn set_theme_changes_later_lookups_only() {
    let root = fixtures();
    let (icons, _) = loader(vec![root.join("user"), root.join("system")], "Child");
    let request = IconRequest::named(["order"], 48, 1.0);
    let child = icons.load(&request);
    child.wait(WAIT);
    icons.set_theme("hicolor");
    assert_eq!(icons.theme(), "hicolor");
    let hicolor = icons.load(&request);
    hicolor.wait(WAIT);
    // The fixture files are empty, so they fail to decode; the paths are what matter.
    assert!(
        child
            .path()
            .is_some_and(|p| p.ends_with("Grand/48x48/apps/order.png"))
    );
    assert!(
        hicolor
            .path()
            .is_some_and(|p| p.ends_with("hicolor/48x48/apps/order.png"))
    );
    assert_eq!((child.theme(), hicolor.theme()), ("Child", "hicolor"));
}

#[test]
fn rescan_finds_a_new_icon_and_drops_stale_handles() {
    let scratch = Scratch::new("loader");
    scratch.put("T/index.theme", "[Icon Theme]\nDirectories=scalable/apps\n[scalable/apps]\nSize=16\nType=Scalable\nMinSize=1\nMaxSize=512\n");
    scratch.put("T/scalable/apps/first.svg", "");
    let (icons, wakes) = loader(vec![scratch.root().to_path_buf()], "T");
    let request = IconRequest::named(["late"], 22, 1.0);
    let before = icons.load(&request);
    assert_eq!(before.wait(WAIT), IconState::NotFound);
    assert!(woken(&wakes, 1));
    assert!(icons.poll());
    // Nothing changed: a rescan keeps the handles and doesn't wake the view.
    icons.rescan();
    let same = icons.load(&request);
    same.wait(WAIT);
    assert!(Arc::ptr_eq(&before.0, &same.0));
    assert!(!icons.poll());
    let svg = std::fs::read_to_string(fixtures().join("decode/Decode/scalable/apps/good.svg"));
    scratch.put(
        "T/scalable/apps/late.svg",
        &svg.unwrap_or_else(|e| panic!("good.svg: {e}")),
    );
    icons.rescan();
    assert!(polled(&icons), "a change wakes the view");
    let after = icons.load(&request);
    assert_eq!(after.wait(WAIT), IconState::Ready);
    // The handle kept from before keeps what it found.
    assert_eq!(before.state(), IconState::NotFound);
}

#[test]
fn dropping_the_loader_stops_its_thread_and_keeps_what_was_found() {
    let (icons, _) = decode_loader();
    let icon = icons.load(&IconRequest::named(["good"], 22, 1.0));
    assert_eq!(icon.wait(WAIT), IconState::Ready);
    let alive = Arc::downgrade(&icons.inner.common);
    drop(icons);
    let deadline = Instant::now() + WAIT;
    while alive.strong_count() > 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(alive.strong_count(), 0, "the thread let go of its state");
    assert_eq!(icon.state(), IconState::Ready);
}
