# Wayland Capability Spike (L06)

This document records capture permissions, injection methods, and overlay capabilities for Wayland compositors.

## Matrix

| Feature | Sway / wlroots | KDE Plasma (Wayland) | GNOME (Wayland) |
| --- | --- | --- | --- |
| **Capture Permission** | evdev / udev (Input group) | evdev / udev (Input group) | evdev / udev (Input group) |
| **Injection Method** | uinput or wlr-virtual-pointer | uinput or libei / RemoteDesktop | uinput or libei / RemoteDesktop |
| **Pointer Coordinates** | zwlr_layer_shell_v1 | zwlr_layer_shell_v1 | Mutter-specific / None |
| **Overlay Stacking** | layer-shell (top/overlay) | layer-shell (top/overlay) | Not supported natively |
| **Input Transparency** | Region passthrough | Region passthrough | N/A |
| **Scale & DPI** | Supported | Supported | N/A |
| **Consent Lifetime** | Persistent (udev rules) | Session/Persistent (Portal) | Session (Portal) |

## Implementation Notes

- **Capture & Injection**: evdev/uinput bypasses the compositor entirely for input capture and injection. This is universally supported as long as the user is in the \input\ group.
- **Overlays (Sway/KDE)**: Both wlroots-based compositors and KDE Plasma fully support \gtk-layer-shell\ (or native wayland \zwlr_layer_shell_v1\). We can render our grid overlay here.
- **Overlays (GNOME)**: GNOME explicitly rejects the layer-shell protocol. Drawing a global overlay on GNOME Wayland requires either an extension (like \Burn-My-Windows\ does) or falling back to a regular window that cannot guarantee topmost unmanaged stacking.
