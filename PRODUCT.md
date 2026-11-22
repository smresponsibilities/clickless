# Product

Clickless provides keyboard-driven pointer control through native platform backends. Windows already has Home, Practice, tray, and a separate Settings process.

The shared GPUI shell includes Home, Settings and safe Practice. Platform runtimes retain input, tray, permissions and lifecycle ownership. Source candidate selects the shell by default; native desktop acceptance remains open.

Settings edits activation, movement, grid behavior, pointer shortcuts, and grid appearance. Raw invalid text must survive search and navigation. Apply changes the running session only after acknowledgement. Save uses existing atomic persistence. Standalone Settings can Save for the next launch.

Input capture, native output, permissions, and window focus ownership remain backend responsibilities. No GUI code runs on hook callbacks. Keyboard navigation, accessible control names, error recovery, and safe shutdown are required.
