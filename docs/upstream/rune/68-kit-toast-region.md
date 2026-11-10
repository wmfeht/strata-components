# 68. rune-kit `ToastRegion` and `Toast` are fixed

| | |
| --- | --- |
| Kind | New issue |
| Drafts | U-C16-1 |
| Affects | C16 (progress dock, built on Rune primitives instead) |
| Retires | C16's own dock |
| Checked | By source: `toast.rs:128–130` (`.key("toast-region")`, `.label("Notifications")`) at `5d421fd` and `04b38ae`. Head adds `ToastStack` (`1633f8d6`, Sonner's stack), not checked against these asks. |

Paste from here.

---

**Title:** rune-kit `ToastRegion` and `Toast`: no key, name, insets, height cap or width

At `5d421fd`, `ToastRegion` keys itself `toast-region`, names itself "Notifications"
(`rune-kit/src/toast.rs:128–130`), pins 32 and 48 points in, and has no height cap or scroll;
`Toast` is 320 points wide with a fixed radius and padding. A file manager's progress dock
(several long-running copies, each a card) needs its own name, insets and a list that scrolls.

**Ask.** A key, a name, insets, a maximum height that scrolls, and the width from the caller
(or the same on the new `ToastStack`).
