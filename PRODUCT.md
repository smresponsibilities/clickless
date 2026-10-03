# Product

Clickless provides keyboard-driven pointer control through native platform backends. Windows already has Home, Practice, tray, and a separate Settings process.

This change finishes Settings implementation first, as selected by the owner. Home and Practice retain their current Windows implementation.

Settings edits activation, movement, grid behavior, pointer shortcuts, and grid appearance. Raw invalid text must survive search and navigation. Apply changes the running session only after acknowledgement. Save uses existing atomic persistence. Standalone Settings can Save for the next launch.

Input capture, native output, permissions, and window focus ownership remain backend responsibilities. No GUI code runs on hook callbacks. Keyboard navigation, accessible control names, error recovery, and safe shutdown are required.
