//! Which icon each semantic name draws, the same in every catalog: Strata's own choice where
//! Strata has one (read at lgse/strata `upstream/main` 35ee677ab: its context menus, sidebar
//! and `src/ui/browser/entry.rs`), else the closest Lucide icon.
//!
//! The match is exhaustive, so a name W2 adds fails to build until it has an icon here, and a
//! target that leaves `icons/` fails to build too.

use super::super::IconName;
use super::catalog::index as i;

pub(super) const fn index(name: IconName) -> usize {
    match name {
        // Entries.
        IconName::Folder => i::STRATA_FOLDER,
        IconName::FolderOpen => i::STRATA_FOLDER_OPEN,
        // Strata draws a file of no known type as its document icon.
        IconName::File => i::STRATA_FILE_TEXT,
        IconName::FileText => i::STRATA_FILE_TEXT,
        IconName::FileCode => i::STRATA_FILE_CODE,
        IconName::FileImage => i::STRATA_IMAGE,
        IconName::Symlink => i::STRATA_KIT_LINK,
        IconName::Lock => i::STRATA_LOCK,
        IconName::Archive => i::STRATA_FILE_ARCHIVE,
        IconName::Unlock => i::STRATA_LOCK_OPEN,

        // Places.
        IconName::Home => i::STRATA_HOUSE,
        IconName::Desktop => i::STRATA_MONITOR,
        IconName::Documents => i::STRATA_FILE_TEXT,
        IconName::Downloads => i::STRATA_DOWNLOAD,
        IconName::Music => i::STRATA_MUSIC_2,
        IconName::Pictures => i::STRATA_IMAGE,
        IconName::Videos => i::STRATA_VIDEO,
        IconName::Recent => i::STRATA_CLOCK,
        IconName::Network => i::STRATA_NETWORK,
        IconName::Computer => i::STRATA_MONITOR,

        // Devices: Strata draws every drive, USB stick included, as a hard drive.
        IconName::Drive => i::STRATA_HARD_DRIVE,
        IconName::Usb => i::STRATA_HARD_DRIVE,
        IconName::Optical => i::STRATA_DISC,
        IconName::Phone => i::STRATA_SMARTPHONE,
        IconName::Eject => i::STRATA_EJECT,

        // Pins and the trash: Strata's Unpin shows the pin, and its trash has one icon.
        IconName::Pin => i::STRATA_PIN,
        IconName::Unpin => i::STRATA_PIN,
        IconName::Trash => i::STRATA_TRASH,
        IconName::TrashFull => i::STRATA_TRASH,
        IconName::Grip => i::STRATA_KIT_GRIP_VERTICAL,

        // Navigation.
        IconName::ChevronLeft => i::STRATA_KIT_CHEVRON_LEFT,
        IconName::ChevronRight => i::STRATA_CHEVRON_RIGHT,
        IconName::ChevronUp => i::STRATA_KIT_CHEVRON_UP,
        IconName::ChevronDown => i::STRATA_CHEVRON_DOWN,
        IconName::Back => i::STRATA_ARROW_LEFT,
        IconName::Forward => i::STRATA_ARROW_RIGHT,
        IconName::Up => i::STRATA_ARROW_UP,
        IconName::Refresh => i::STRATA_REFRESH,
        IconName::More => i::STRATA_KIT_ELLIPSIS,
        IconName::Menu => i::STRATA_KIT_MENU,

        // Actions.
        IconName::Plus => i::STRATA_PLUS,
        IconName::Close => i::STRATA_X,
        IconName::Search => i::STRATA_SEARCH,
        IconName::Filter => i::STRATA_FUNNEL,
        IconName::Check => i::STRATA_CHECK,
        IconName::Copy => i::STRATA_COPY,
        IconName::Cut => i::STRATA_SCISSORS,
        IconName::Paste => i::STRATA_CLIPBOARD_PASTE,
        IconName::FolderPlus => i::STRATA_FOLDER_PLUS,
        IconName::Extract => i::STRATA_PACKAGE_OPEN,
        IconName::Terminal => i::STRATA_TERMINAL,
        IconName::Properties => i::STRATA_INFO,
        IconName::Rename => i::STRATA_PENCIL,
        IconName::Open => i::STRATA_EXTERNAL_LINK,
        IconName::Wrap => i::STRATA_WRAP_TEXT,
        IconName::Source => i::STRATA_CODE_XML,
        IconName::Rendered => i::STRATA_EYE,
        IconName::EyeOff => i::STRATA_EYE_OFF,

        // Views.
        IconName::ViewList => i::STRATA_LIST,
        IconName::ViewColumns => i::STRATA_COLUMNS,
        IconName::ViewIcons => i::STRATA_ICONS,
        IconName::Sidebar => i::STRATA_PANEL_LEFT_SYMBOLIC,
        IconName::SortAscending => i::STRATA_ARROW_UP_NARROW_WIDE,
        IconName::SortDescending => i::STRATA_ARROW_DOWN_WIDE_NARROW,

        // Status.
        IconName::Info => i::STRATA_INFO,
        IconName::Warning => i::STRATA_TRIANGLE_ALERT,
        IconName::Error => i::STRATA_CIRCLE_X,
        IconName::Success => i::STRATA_CIRCLE_CHECK,
        IconName::Keyboard => i::STRATA_KEYBOARD,
        IconName::Help => i::STRATA_KIT_HELP,
        IconName::Clipboard => i::STRATA_KIT_CLIPBOARD,
        IconName::Settings => i::STRATA_SETTINGS,

        // Media.
        IconName::Play => i::STRATA_PLAY,
        IconName::Pause => i::STRATA_PAUSE,
        IconName::SkipBack => i::STRATA_SKIP_BACK,
        IconName::SkipForward => i::STRATA_SKIP_FORWARD,
        IconName::Volume => i::STRATA_VOLUME_2,
        IconName::VolumeMute => i::STRATA_VOLUME_X,
        IconName::Repeat => i::STRATA_KIT_REPEAT,
        IconName::Fullscreen => i::STRATA_MAXIMIZE,

        // Viewers.
        IconName::ZoomIn => i::STRATA_KIT_ZOOM_IN,
        IconName::ZoomOut => i::STRATA_KIT_ZOOM_OUT,
        IconName::Rotate => i::STRATA_KIT_ROTATE_CW,
        IconName::Fit => i::STRATA_KIT_SCAN,
    }
}
