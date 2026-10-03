# Settings design

Owner selected Tusk's GPUI library family with Inlark's visual direction. Use pinned gpui-kit 0.6.6. Do not copy Inlark implementation.

Settings uses quiet neutral surfaces, proportional system typography, aligned label/help pairs, restrained lavender focus and selection, and complete light/dark palettes. Six stable navigation items correspond to existing descriptors. Avoid per-field cards and decorative animation.

Page titles use 26 logical pixels; labels use 14; help uses 13. Navigation rows use 36-pixel initial height. Wide editor controls align at 220 pixels. Narrow windows stack label and editor, and footer actions wrap. Minimum window size is 640 by 480.

System, Light, and Dark control the Settings session appearance. They do not edit persisted grid colors. Grid appearance controls change the configuration draft only.

Keep search and actions reachable. Preserve raw input across page/filter changes. Error summaries name the failure. Applying and saving disable further edits until acknowledgement completes. Failed persistence permits retry.

Native visual, scale, high-contrast, and Narrator acceptance remain required before selecting GPUI as the default Windows host.
