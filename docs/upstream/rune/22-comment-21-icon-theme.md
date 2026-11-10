# 22. Comment on #21: what freedesktop icon lookup needs for a file manager

| | |
| --- | --- |
| Kind | Comment on [#21](https://github.com/anomalyco/rune/issues/21) (app icons off macOS) |
| Drafts | U-P6-1 |
| Affects | P6 (icon theme lookup), C21 (Open With), file-type icons everywhere |
| Retires | P6's GTK-4-style lookup (search path, `Inherits`, `index.theme` sizes, its worker thread and LRU) |
| Checked | By source: `freedesktop::icon_file` scans `icons/hicolor` only (`image.rs:481–489`) at `5d421fd`; `image.rs` changed past the pin (34 lines) with no theme lookup. The fixture below is from P6; not run here. |

Paste from here.

---

A file manager on Rune shows other apps' icons (Open With) and MIME-type icons, and hits
these limits of `ImageCache::app_icon` on Linux at `5d421fd`:

- **The user's theme.** `freedesktop::icon_file` (`image.rs:481`) scans `icons/hicolor` only:
  no current theme and no `Inherits`, so an icon that ships only in Adwaita, Yaru or Breeze isn't
  found.
- **`index.theme` sizes.** It reads a size from the directory name (`48x48`, `@2`), not the
  theme's `Size`, `Type`, `Threshold`, `MinSize`, `MaxSize` and `Scale`.
- **Lookup by name.** It takes a `.desktop` path only, so a MIME icon name (`text-x-python`) or a
  list of fallback names (as `GThemedIcon` gives) can't be looked up.
- **Not found.** A missing icon and a broken file both resolve to `Image::error`.
- It lists every size directory again on each call.

Headless, with `XDG_DATA_HOME=$T/share`:

```sh
mkdir -p $T/share/icons/Fixture/48x48/apps $T/share/applications
printf '[Icon Theme]\nName=Fixture\nInherits=hicolor\nDirectories=48x48/apps\n\n[48x48/apps]\nSize=48\nType=Fixed\n' \
  > $T/share/icons/Fixture/index.theme
cp any-48px.png $T/share/icons/Fixture/48x48/apps/fixture-app.png
printf '[Desktop Entry]\nType=Application\nName=Fixture\nExec=true\nIcon=fixture-app\n' \
  > $T/share/applications/fixture.desktop
```

`ImageCache::new(waker).app_icon_scaled("$T/share/applications/fixture.desktop", 48.0, 1.0)`
returns `Err("no icon for …")` (by source), and nothing names a theme to look in.

What would cover it: an image source taking the theme by name,
`ImageCache::themed_icon(names, theme, size_pt, scale)`, following the Icon Theme
Specification, with "not found" apart from a decode error. Or, smaller: a way to run a path
lookup on the cache's worker (`cache.file_with(|| -> Option<PathBuf>, size_pt, scale)`), so a
crate doing the lookup needs no thread of its own.
