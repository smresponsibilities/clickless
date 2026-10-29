# Wayland Support
Status: Unsupported

Clickless relies on X11 geometry (`Xinerama`, `Xrandr`) for pointer output and `evdev` for exclusive keyboard capture. 
Wayland's security model fundamentally restricts global pointer positioning and exclusive key grabbing without privileged compositor plugins.
Wayland is explicitly fail-fast in Clickless. Users must run an X11 session.
