# Keyboard usability checks, 2026-10-03

Published WinUI migration as 063b242. Ran twenty native CapsLock tap/release rounds with Clickless active in an isolated Notepad tab. Alternated lowercase a and Shift+h uppercase H. Ctrl+A checked in rounds 1, 10, 20; Ctrl+Shift+Left checked in rounds 5, 15. Expected editing behavior observed throughout. Intermittent CapsLock sticking was not reproduced.

Existing core checks exposed delayed Shift typing activating grid before the next letter. Built-in Shift now activates only on a short unused tap; held Shift reaches ordinary typing. Explicit Shift leader retains configured hold behavior. Existing test updated, failed before fix, passed afterward.

Native Settings search exposed a WinRT string ItemsSource crash. Suggestions now use boxed strings. Programmatic query changes cancel debounce; advanced groups default expanded so navigation redraws keep selected controls visible. Second launch opens existing Settings. Home/Practice reuse DPI-scaled centered placement. Theme applies to outer caption shell and content.

Windows CI lacked Windows App SDK runtime. Workflow now installs official Microsoft runtime after signature verification. Linux/macOS passed original run; follow-up CI must verify Windows repair.

Local workspace build, strict workspace and fallback Clippy, formatting, and full existing test suite run with explorer bounded to 500 seeds x 500 events. No new regression cases added under owner waiver.

Limits: automation supports taps/chords, not sustained physical holds. Full held-key Practice, sustained motion/scroll/drag cleanup, mixed DPI/display unplug, and actual Narrator speech remain unverified. Twenty rounds do not prove intermittent CapsLock failure absent. Original configuration bytes preserved. Audit-created theme preference removed.
