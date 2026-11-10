# 26. Linux: play app-decoded PCM with a playback clock, without `Player`

| | |
| --- | --- |
| Kind | New issue (related #30, #100, #112) |
| Drafts | U-P7-1 |
| Affects | P7 (media audio), C13 (media player) |
| Retires | P7's GStreamer `appsrc` sink and its clock |
| Checked | By source: no PCM output API at `5d421fd` or `04b38ae`; `Player` still has no Linux backend. No program needed. |

Paste from here.

---

**Title:** Linux: play app-decoded PCM with a playback clock

Context: a file manager previews media by decoding in a sandboxed helper that streams RGBA
frames and PCM back. The frames already work (`Pixels::set` from any thread, no view commit).
The sound has nowhere to go: `Player` takes a file or URL and has no Linux backend (#30, #100,
#112), and Rune has no API that takes PCM.

**Steps.** At `5d421fd` (or `04b38ae`), look for a way to play a buffer of interleaved S16 stereo
at 48 kHz and read back how much of it has been heard: there is none outside `Player`, whose
Linux backend reports "unsupported".

**Ask.** A small output: open it with a sample format, rate and channel count; push interleaved
PCM; play, pause and flush; read a position that follows the device (what has been heard, its
delay counted); and a callback or waker for underruns, the end, and a lost device. Video then
paces on that position. A Linux backend for `Player` wouldn't cover it: the app has PCM, not a
file.

Related: #30, #112, #506 (`Player` volume).
