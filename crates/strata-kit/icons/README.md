# Icon sources

strata-kit's `build.rs` turns these SVGs into the icon catalogs in `src/icons/`
(`lucide::SET`, `catppuccin::MOCHA`, `catppuccin::LATTE`). Each file's stem is its icon's
`Icon::name`, so Strata's own icon names (`strata-folder`, `strata-ctp-rust`) find the same
icons here.

| Folder | What | From |
| --- | --- | --- |
| `lucide/` | 110 Lucide interface and generic file-type icons, Strata's `strata-*.svg` | `data/icons/scalable/actions/` in [lgse/strata](https://github.com/lgse/strata) at `35ee677ab0a633d5d49055464f2946fcf4995ae7`, byte for byte. Strata took them from [Lucide](https://github.com/lucide-icons/lucide) 1.35.0. |
| `catppuccin/` | 354 Catppuccin file-type icons for code, language and config files, `strata-ctp-*.svg` | `data/icons/catppuccin/` in lgse/strata at the same revision, byte for byte. Strata copied them from [catppuccin/vscode-icons](https://github.com/catppuccin/vscode-icons) `b6915da9f6889b683a110aa747de96c2820a537d` (v1.26.0), `icons/css-variables/`. |
| `lucide-extra/` | 10 Lucide icons Strata doesn't ship, renamed `strata-kit-<lucide name>.svg` | `https://raw.githubusercontent.com/lucide-icons/lucide/1.35.0/icons/<name>.svg` (tag `1.35.0`): `clipboard`, `ellipsis`, `grip-vertical`, `link`, `menu`, `repeat`, `rotate-cw`, `scan`, `zoom-in`, `zoom-out`. Byte for byte apart from the name. `circle-help` isn't at that tag, so Help uses `own/strata-kit-help.svg`. |
| `own/` | `strata-kit-chevron-left` and `-up`: Strata's Lucide chevron-right and -down turned half a turn. `strata-kit-help`: this repository's own art. | This repository |

The Mocha and Latte palettes in `build.rs` are Strata's (`src/assets.rs` at the same revision),
from the Catppuccin palette.

## What the build changes

The SVG files stay as they are; `build.rs` changes the bodies it generates, following Strata's
own renderer (`src/assets.rs`):

- Paint attributes on the root `<svg>` move to a wrapping `<g>`, as Rune's icon bodies have no
  `<svg>`.
- Strata's tint placeholders in `lucide/` (`#8bc9eb`, `#22d3ee`, `#2e3436`, and `#ffffff` in
  `strata-check-on-primary`) become `currentColor`, so components tint them. `strata-folder`
  gets Strata's 15 % fill in its own colour.
- Each `var(--vscode-ctp-*)` colour in `catppuccin/` resolves three ways: the Mocha colour, the
  Latte colour, or `currentColor` with a 15 % fill on unfilled shapes (Strata's Theme colors).

## Licences and credits

Each notice below must ship with any copy of these icons, including a binary that embeds
strata-kit. The crate's own licence stays MIT; the data keeps its own licences.

- **Lucide**: ISC, Copyright (c) 2026 Lucide Icons and Contributors. The icons Lucide derived
  from Feather are MIT, Copyright (c) 2013-present Cole Bemis; the file lists them. Full text,
  both notices: [`LICENSE-Lucide-ISC.txt`](LICENSE-Lucide-ISC.txt), from Strata's
  `data/licenses/Lucide-ISC.txt`.
- **Catppuccin VSCode Icons**: MIT, Copyright (c) 2023 Catppuccin and Copyright (c) 2023
  thang-nm. Full text: [`LICENSE-Catppuccin-VSCode-Icons-MIT.txt`](LICENSE-Catppuccin-VSCode-Icons-MIT.txt).
- **Strata**: MIT, Copyright (c) 2026 LGSE Ltd. Its curation, its edits to Lucide (the outlined
  `-symbolic` icons and the filled sort caret) and its rendering rules. Full text:
  [`LICENSE-Strata.txt`](LICENSE-Strata.txt).

## Refreshing

1. Copy `data/icons/scalable/actions/*.svg` and `data/icons/catppuccin/*.svg` from a newer
   Strata revision over `lucide/` and `catppuccin/` (remove files Strata removed), and update
   the revisions above and the licence files from Strata's `data/licenses/`.
2. Build. `build.rs` stops on anything it doesn't know: a root attribute, a colour that isn't a
   tint placeholder in `lucide/`, a Catppuccin colour without a palette entry, a name in two
   folders. `src/icons/lucide/semantic.rs` fails to compile when an icon it maps is gone.
3. Run `mise run test icons::` and `mise run test k3`, then bless and review the `k3` goldens.
