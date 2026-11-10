# 51. rune-kit scroll bars over viewports keyed alike share state and drag the first

| | |
| --- | --- |
| Kind | New issue |
| Drafts | R-C24-1 |
| Affects | C24 (its own overlay bar, a deviation from the plan) |
| Retires | One reason C24 keeps its own scroll bar instead of rune-kit's |
| Checked | By source: `kept(&format!("{viewport_key}-scroll-area"))` (`rune-kit/src/scroll_bar.rs:799` at `5d421fd`, `:757` at `04b38ae`; `kept` itself at `:365` at the pin). C24's probe at the pin; not re-run. |

Paste from here.

---

**Title:** rune-kit `ScrollBar`: two viewports with the same key share a bar's state, and dragging one scrolls the first

`ScrollBar::scroll_bar_styled` keys its kept state by `"{key}-scroll-area"` (`kept`,
`rune-kit/src/scroll_bar.rs:799` at `5d421fd`) and drags the target `"{area}/{key}"`, which
resolves to the first match in tree order.

**Repro** (headless): two panes keyed `a` and `b`, each holding a 1,000-row virtual viewport
keyed `viewport` with `.scroll_bar_styled(ScrollBarStyle::scroll_view(&kit))`; wheel `b` to
2,000, then drag `b`'s thumb down 120 pt: `a` scrolls to 7,956 and `b` stays at 2,000.

**Ask.** Key the state and the target by the viewport's path, or take an explicit key.
