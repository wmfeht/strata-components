# 84. Comment on #114: list the installed families

| | |
| --- | --- |
| Kind | Comment on [#114](https://github.com/anomalyco/rune/issues/114) (which #226 links) |
| Drafts | U16 (S5) |
| Affects | K2 (font picker) |
| Retires | K2's font list passed in as data |
| Checked | `text/system_fonts.rs:149` `pub(super) fn installed_family_files` at `5d421fd` and `04b38ae`. Names resolve since `99c123d` (#245), before the pin. |

Paste from here.

---

Since `99c123d` (#245) a family named in a slot resolves through fontconfig, which covers the
first ask. A font picker also needs the list of installed families to offer, and none is public:
`text::system_fonts::installed_family_files` is `pub(super)` (`text/system_fonts.rs:149`), and the
lookup collection it builds isn't exposed. Could Rune add
`rune::text::installed_families() -> Vec<String>` (fontconfig on Linux, CoreText and
DirectWrite elsewhere), ideally with whether each is monospaced, so a picker can offer only
monospaced families for the mono slot?
