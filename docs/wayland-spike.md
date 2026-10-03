# Wayland capability spike, L06

Status: discovery incomplete. Clickless rejects Wayland sessions before input setup or keyboard grab, including sessions with XWayland. Current Linux overlay uses X11. No Wayland compositor passed native acceptance.

| Session | Current Clickless behavior | Required proof |
| --- | --- | --- |
| Sway / wlroots | Startup rejected | Capture consent, output injection, overlay placement, coordinates and cleanup |
| KDE Plasma Wayland | Startup rejected | Same checks on a recorded Plasma/protocol version |
| GNOME Wayland | Startup rejected | Same checks; no layer-shell support assumption |

## Discovery checks

Check capture, pointer injection and overlay placement separately. evdev/uinput access requires physical event-device and virtual-device permissions. It does not prove usable compositor coordinates or overlays. Input group membership grants other processes running as that user broad input access; do not recommend it as application isolation or silently change permissions.

Layer-shell positions surfaces. It does not provide a global pointer-coordinate query or input-injection permission. Record advertised compositor protocols, output origins/scales, and how selected overlay coordinates map to the chosen injection API. XWayland availability does not establish global Wayland overlay support.

RemoteDesktop portals and libei need separate consent, session lifetime and API/version checks. Verify permission denial/revocation, screen lock, compositor restart, monitor disconnect, and release of every held key/button. No capability is supported merely because a protocol exists.

After P01/L01 safety acceptance, test each compositor in an isolated desktop session. Record OS/compositor version, exact API, permission path, expected and observed behavior, and recovery. Only then choose an implementation and advertise support per compositor.

References for discovery: [Wayland protocols](https://wayland.app/protocols/), [RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html), [Linux uinput](https://www.kernel.org/doc/html/latest/input/uinput.html). These links are research starting points, not acceptance evidence.
