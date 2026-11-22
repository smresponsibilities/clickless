import math
import os
from PIL import Image, ImageDraw, ImageFont

# --- Configuration (960x540 16:9, 25 FPS = exact 40ms GIF delay) ---
WIDTH = 960
HEIGHT = 540
FPS = 25
TOP_BAR_H = 24
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
OUT_PATH = os.path.join(SCRIPT_DIR, "demo.gif")

RESAMPLE_LANCZOS = getattr(Image, "Resampling", Image).LANCZOS
QUANTIZE_MEDIANCUT = getattr(Image, "Quantize", Image).MEDIANCUT
DITHER_NONE = getattr(Image, "Dither", Image).NONE

# --- Verified Real-World Design Tokens ---
C_DESKTOP = (18, 20, 24)        # #121418 clean matte dark backdrop
C_MENUBAR = (28, 28, 30)        # #1C1C1E macOS dark menu bar

# macOS Terminal (Left)
C_TERM_BG = (30, 30, 30)        # #1E1E1E macOS dark terminal body
C_TERM_BAR = (45, 45, 47)       # #2D2D2F macOS dark titlebar
C_TERM_BORDER = (68, 68, 72)    # #444448 subtle macOS window hairline
C_TERM_DIM = (54, 54, 58)

# macOS Traffic Lights
C_TL_RED = (255, 95, 86)        # #FF5F56
C_TL_YELLOW = (255, 189, 46)    # #FFBD2E
C_TL_GREEN = (39, 201, 63)      # #27C93F
C_TL_INACTIVE = (76, 76, 80)

# Chrome Dark Mode (Right)
C_CHR_STRIP = (32, 33, 36)      # #202124 tab strip & omnibox
C_CHR_TOOLBAR = (53, 54, 58)    # #35363A active tab & toolbar
C_CHR_ICON = (189, 193, 198)    # #BDC1C6 ghost toolbar icons

# GitHub Dark Default / Primer Tokens (Right Page)
C_GH_BG = (13, 17, 23)          # #0D1117 canvas.default
C_GH_SURFACE = (22, 27, 34)     # #161B22 canvas.subtle
C_GH_BORDER = (48, 54, 61)      # #30363D border.default
C_GH_TEXT = (230, 237, 243)     # #E6EDF3 fg.default
C_GH_MUTED = (139, 148, 158)    # #8B949E fg.muted
C_GH_GREEN_BTN = (35, 134, 54)  # #238636 btn.primary.bg
C_GH_GREEN = (63, 185, 80)      # #3FB950 success.fg
C_GH_BLUE = (47, 129, 247)      # #2F81F7 accent.fg
C_GH_AMBER = (210, 153, 34)     # #D29922 attention.fg
C_GH_PURPLE = (137, 87, 229)    # #8957E5 done.emphasis (Merged)
C_GH_CYAN = (121, 192, 255)     # #79C0FF syntax cyan
C_GH_ORANGE = (247, 129, 102)   # #F78166 active tab underline

# Grid & Subgrid Accent
C_GRID_BLUE = (47, 129, 247)
C_SUB_CYAN = (86, 212, 221)

# --- Grid & Subgrid Geometry (4x3 Macro -> 3x3 Subgrid) ---
GRID_COLS, GRID_ROWS = 4, 3
SUB_COLS, SUB_ROWS = 3, 3
GRID_LABELS = [
    ["AA", "AS", "AD", "AF"],
    ["SA", "SS", "SD", "SF"],
    ["DA", "DS", "DD", "DF"],
]
SUB_LABELS = [
    ["Q", "W", "E"],
    ["A", "S", "D"],
    ["Z", "X", "C"],
]

GW = WIDTH / GRID_COLS                  # 240.0
GH = (HEIGHT - TOP_BAR_H) / GRID_ROWS   # 172.0
SW = GW / SUB_COLS                      # 80.0
SH = GH / SUB_ROWS                      # 57.333...


def subcell_center(mr, mc, sr, sc):
    return (
        mc * GW + (sc + 0.5) * SW,
        TOP_BAR_H + mr * GH + (sr + 0.5) * SH,
    )


# Verified subgrid centers matching real UI positions
P_RELOAD = subcell_center(0, 2, 1, 0)   # AD -> A = (520.0, 110.0) Chrome Reload ghost icon
P_CHECKS = subcell_center(1, 2, 0, 2)   # SD -> E = (680.0, 224.7) GitHub Checks tab
P_MERGE = subcell_center(2, 2, 0, 1)    # DD -> W = (600.0, 396.7) GitHub Merge PR button
P_TERM = subcell_center(1, 0, 1, 2)     # SA -> D = (200.0, 282.0) macOS Terminal body

L_BOX = (24, 48, 468, 476)    # 444 x 428 macOS Terminal
R_BOX = (484, 48, 936, 476)   # 452 x 428 Chrome + GitHub

TYPED_CMD = "git pull origin main"
PROMPT_TEXT = "shivam@mac clickless % "


# --- Fonts ---
def get_font(size, bold=False, mono=False):
    if mono:
        names = (
            ["consolab.ttf", "JetBrainsMono-Bold.ttf", "DejaVuSansMono-Bold.ttf", "Menlo.ttc"]
            if bold
            else ["consola.ttf", "JetBrainsMono-Regular.ttf", "DejaVuSansMono.ttf", "Menlo.ttc"]
        )
    else:
        names = (
            ["segoeuib.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf", "Helvetica.ttc"]
            if bold
            else ["segoeui.ttf", "arial.ttf", "DejaVuSans.ttf", "Helvetica.ttc"]
        )
    for name in names:
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


FONT_UI = get_font(12)
FONT_UI_BOLD = get_font(12, bold=True)
FONT_SM = get_font(11)
FONT_SM_BOLD = get_font(11, bold=True)
FONT_TITLE = get_font(15, bold=True)
FONT_MONO = get_font(12, mono=True)
FONT_MONO_BOLD = get_font(12, bold=True, mono=True)
FONT_GRID = get_font(16, bold=True, mono=True)
FONT_SUBGRID = get_font(13, bold=True, mono=True)
FONT_KEY = get_font(12, bold=True, mono=True)
FONT_BADGE = get_font(11, bold=True, mono=True)


# --- Helpers ---
def clamp(v, lo=0.0, hi=1.0):
    return max(lo, min(hi, v))


def lerp(a, b, t):
    return a + (b - a) * t


def ease_out_cubic(t):
    t = clamp(t)
    return 1.0 - (1.0 - t) ** 3


def ease_out_expo(t):
    t = clamp(t)
    if t <= 0.0:
        return 0.0
    if t >= 1.0:
        return 1.0
    return (1.0 - math.pow(2.0, -10.0 * t)) / (1.0 - math.pow(2.0, -10.0))


def draw_centered_text(draw, cx, cy, text, font, fill):
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    draw.text((cx - tw / 2 - bbox[0], cy - th / 2 - bbox[1]), text, font=font, fill=fill)


# --- 4x Supersampled Vector Icon Sprites ---
def create_reload_sprite(color):
    """Crisp anti-aliased Chrome circular reload arrow (16x16 from 64x64)."""
    hi = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    col = (*color, 255)
    # Smooth 295-degree circular ring (gap at upper-right)
    d.arc([14, 14, 50, 50], start=25, end=318, fill=col, width=6)
    # Clean clockwise arrowhead seated right on the 318-degree arc tip
    d.polygon([(48.5, 12.0), (49.5, 25.0), (37.0, 23.0)], fill=col)
    return hi.resize((16, 16), RESAMPLE_LANCZOS)


def create_back_sprite(color):
    """Crisp anti-aliased Chrome left arrow (16x16 from 64x64)."""
    hi = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    col = (*color, 255)
    d.line([(17, 32), (48, 32)], fill=col, width=5)
    d.line([(29, 20), (17, 32), (29, 44)], fill=col, width=5, joint="curve")
    return hi.resize((16, 16), RESAMPLE_LANCZOS)


def create_check_sprite(color):
    """Crisp anti-aliased GitHub check badge (14x14 from 56x56)."""
    hi = Image.new("RGBA", (56, 56), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    d.ellipse([4, 4, 52, 52], fill=(*color, 255))
    d.line([(16, 29), (24, 38), (40, 20)], fill=(*C_GH_BG, 255), width=6, joint="curve")
    return hi.resize((14, 14), RESAMPLE_LANCZOS)


def create_cursor_sprite():
    scale = 4
    cw, ch = 22, 26
    hi = Image.new("RGBA", (cw * scale, ch * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(hi, "RGBA")
    ox, oy = 2, 2
    pts = [
        (ox, oy), (ox, oy + 16), (ox + 4.2, oy + 12.6),
        (ox + 7.4, oy + 19.5), (ox + 10.2, oy + 18.2),
        (ox + 7.0, oy + 11.4), (ox + 12.2, oy + 11.4),
    ]
    pts_hi = [(px * scale, py * scale) for px, py in pts]
    draw.polygon([(px + 3, py + 5) for px, py in pts_hi], fill=(0, 0, 0, 110))
    draw.polygon(pts_hi, fill=(255, 255, 255, 255), outline=(14, 14, 16, 255), width=scale)
    return hi.resize((cw, ch), RESAMPLE_LANCZOS), (ox, oy)


def create_desktop():
    img = Image.new("RGBA", (WIDTH, HEIGHT), (*C_DESKTOP, 255))
    draw = ImageDraw.Draw(img, "RGBA")

    draw.rectangle([0, 0, WIDTH, TOP_BAR_H], fill=(*C_MENUBAR, 255))
    draw.line([(0, TOP_BAR_H - 1), (WIDTH, TOP_BAR_H - 1)], fill=(48, 48, 52, 255), width=1)
    draw.text((18, 5), "Terminal", font=FONT_SM_BOLD, fill=C_GH_TEXT)
    draw.text((80, 5), "Shell", font=FONT_SM, fill=C_GH_MUTED)
    draw.text((120, 5), "Edit", font=FONT_SM, fill=C_GH_MUTED)
    draw.text((156, 5), "View", font=FONT_SM, fill=C_GH_MUTED)
    draw.text((196, 5), "Window", font=FONT_SM, fill=C_GH_MUTED)
    draw.text((WIDTH - 76, 5), "Fri 14:40", font=FONT_SM, fill=C_GH_TEXT)
    return img


def draw_traffic_lights(d, x0, cy, active=True):
    cols = (C_TL_RED, C_TL_YELLOW, C_TL_GREEN) if active else (C_TL_INACTIVE, C_TL_INACTIVE, C_TL_INACTIVE)
    for i in range(len(cols)):
        cx = x0 + i * 20
        d.ellipse([cx - 6, cy - 6, cx + 6, cy + 6], fill=(*cols[i], 255))


# --- Split-Screen Workspace ---
def draw_split_workspace(frame, state, icons):
    ov = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")

    # ==========================================================
    # LEFT WINDOW: Authentic macOS Terminal (#1E1E1E)
    # ==========================================================
    lx0, ly0, lx1, ly1 = L_BOX
    l_focused = not state["focus_right"]

    d.rounded_rectangle([lx0 - 2, ly0 + 4, lx1 + 2, ly1 + 8], radius=12, fill=(0, 0, 0, 85))
    d.rounded_rectangle([lx0, ly0, lx1, ly1], radius=10, fill=(*C_TERM_BG, 255), outline=(*C_TERM_BORDER, 255), width=1)

    th = 32
    d.rounded_rectangle([lx0 + 1, ly0 + 1, lx1 - 1, ly0 + th], radius=9, fill=(*C_TERM_BAR, 255))
    d.rectangle([lx0 + 1, ly0 + th - 9, lx1 - 1, ly0 + th], fill=(*C_TERM_BAR, 255))
    d.line([(lx0 + 1, ly0 + th), (lx1 - 1, ly0 + th)], fill=(*C_TERM_DIM, 255), width=1)
    draw_traffic_lights(d, lx0 + 20, ly0 + 16, active=l_focused)
    draw_centered_text(d, (lx0 + lx1) / 2, ly0 + 16, "zsh — clickless", FONT_UI_BOLD, C_GH_TEXT if l_focused else C_GH_MUTED)

    col_job = lx0 + 18
    col_target = lx0 + 186
    col_stat = lx0 + 344
    prompt_w = d.textbbox((0, 0), PROMPT_TEXT, font=FONT_MONO)[2]

    y = ly0 + 48
    d.text((col_job, y), PROMPT_TEXT, font=FONT_MONO, fill=C_GH_MUTED)
    d.text((col_job + prompt_w, y), "git push origin fix/hidpi", font=FONT_MONO, fill=C_GH_TEXT)
    y += 22
    d.text((col_job, y), "To github.com:smresponsibilities/clickless.git", font=FONT_MONO, fill=C_GH_MUTED)
    y += 20
    d.text((col_job, y), "   a91c4e2..f38b10d  fix/hidpi -> fix/hidpi", font=FONT_MONO, fill=C_GH_CYAN)
    y += 30

    d.text((col_job, y), "WORKFLOW", font=FONT_SM_BOLD, fill=C_GH_MUTED)
    d.text((col_target, y), "TARGET", font=FONT_SM_BOLD, fill=C_GH_MUTED)
    d.text((col_stat, y), "STATUS", font=FONT_SM_BOLD, fill=C_GH_MUTED)
    y += 18
    d.line([(lx0 + 18, y), (lx1 - 18, y)], fill=(*C_TERM_DIM, 255), width=1)
    y += 12

    ci_ok = state["reloaded"]
    ci_rows = [
        ("cargo test", "x86_64-linux", "PASS  12s" if ci_ok else "RUN   08s", C_GH_GREEN if ci_ok else C_GH_AMBER),
        ("cargo test", "aarch64-macos", "PASS  14s" if ci_ok else "RUN   09s", C_GH_GREEN if ci_ok else C_GH_AMBER),
        ("cargo clippy", "workspace", "PASS   4s", C_GH_GREEN),
    ]
    for job, tgt, stat, scol in ci_rows:
        d.text((col_job, y), job, font=FONT_MONO, fill=C_GH_TEXT)
        d.text((col_target, y), tgt, font=FONT_MONO, fill=C_GH_MUTED)
        d.text((col_stat, y), stat, font=FONT_MONO_BOLD, fill=scol)
        y += 24

    # Interactive prompt with live character-by-character typing
    y += 14
    typed_str = TYPED_CMD[:state["typed_chars"]]
    d.text((col_job, y), PROMPT_TEXT, font=FONT_MONO, fill=C_GH_MUTED)
    d.text((col_job + prompt_w, y), typed_str, font=FONT_MONO, fill=C_GH_TEXT)

    if state["term_executed"]:
        y += 22
        d.text((col_job, y), "Updating a91c4e2..f38b10d", font=FONT_MONO, fill=C_GH_MUTED)
        y += 20
        d.text((col_job, y), "Fast-forward  src/grid.rs | 18 +++++++---", font=FONT_MONO, fill=C_GH_GREEN)
        y += 26
        d.text((col_job, y), PROMPT_TEXT, font=FONT_MONO, fill=C_GH_MUTED)
        if l_focused:
            d.rectangle([col_job + prompt_w + 1, y + 1, col_job + prompt_w + 8, y + 14], fill=(*C_GH_TEXT, 255))
    elif l_focused:
        typed_w = d.textbbox((0, 0), typed_str, font=FONT_MONO)[2] if typed_str else 0
        cx_cur = col_job + prompt_w + typed_w + 1
        d.rectangle([cx_cur, y + 1, cx_cur + 7, y + 14], fill=(*C_GH_TEXT, 255))

    # ==========================================================
    # RIGHT WINDOW: Chrome Dark Mode + GitHub Dark Default
    # ==========================================================
    rx0, ry0, rx1, ry1 = R_BOX
    r_focused = state["focus_right"]

    d.rounded_rectangle([rx0 - 2, ry0 + 4, rx1 + 2, ry1 + 8], radius=12, fill=(0, 0, 0, 85))
    d.rounded_rectangle([rx0, ry0, rx1, ry1], radius=10, fill=(*C_GH_BG, 255), outline=(*C_TERM_BORDER, 255), width=1)

    # Chrome Tab Strip (y = 48..90)
    d.rounded_rectangle([rx0 + 1, ry0 + 1, rx1 - 1, ry0 + 42], radius=9, fill=(*C_CHR_STRIP, 255))
    d.rectangle([rx0 + 1, ry0 + 28, rx1 - 1, ry0 + 42], fill=(*C_CHR_STRIP, 255))
    draw_traffic_lights(d, rx0 + 20, ry0 + 21, active=r_focused)

    # Active Chrome Tab
    d.rounded_rectangle([rx0 + 80, ry0 + 10, rx0 + 258, ry0 + 42], radius=8, fill=(*C_CHR_TOOLBAR, 255))
    d.rectangle([rx0 + 80, ry0 + 30, rx0 + 258, ry0 + 42], fill=(*C_CHR_TOOLBAR, 255))
    d.text((rx0 + 94, ry0 + 19), "PR #142 · clickless", font=FONT_SM_BOLD, fill=C_GH_TEXT)
    d.text((rx0 + 240, ry0 + 18), "x", font=FONT_SM, fill=C_GH_MUTED)
    d.text((rx0 + 276, ry0 + 19), "crates.io", font=FONT_SM, fill=C_GH_MUTED)

    # Chrome Toolbar (y = 90..130, center = 110.0 = P_RELOAD[1])
    tb_y = ry0 + 42
    d.rectangle([rx0 + 1, tb_y, rx1 - 1, tb_y + 40], fill=(*C_CHR_TOOLBAR, 255))
    d.line([(rx0 + 1, tb_y + 40), (rx1 - 1, tb_y + 40)], fill=(*C_GH_BORDER, 255), width=1)

    # Supersampled Chrome Back & Reload Ghost Icons
    ov.alpha_composite(icons["back"], (rx0 + 10, int(P_RELOAD[1] - 8)))
    if state["reload_flash"]:
        d.ellipse([P_RELOAD[0] - 13, P_RELOAD[1] - 13, P_RELOAD[0] + 13, P_RELOAD[1] + 13], fill=(74, 76, 82, 255))
    reload_ico = icons["reload_active"] if state["reload_flash"] else icons["reload"]
    ov.alpha_composite(reload_ico, (int(P_RELOAD[0] - 8), int(P_RELOAD[1] - 8)))

    # Chrome Omnibox (#202124 pill)
    d.rounded_rectangle([rx0 + 58, tb_y + 7, rx1 - 14, tb_y + 33], radius=13, fill=(*C_CHR_STRIP, 255))
    d.text((rx0 + 74, tb_y + 13), "github.com/smresponsibilities/clickless/pull/142", font=FONT_SM, fill=C_GH_MUTED)

    # --- GitHub Dark Default Page (#0D1117) ---
    d.text((rx0 + 18, 144), "Fix HiDPI subgrid offset", font=FONT_TITLE, fill=C_GH_TEXT)
    d.text((rx0 + 196, 144), "#142", font=FONT_TITLE, fill=C_GH_MUTED)

    if state["merged"]:
        d.rounded_rectangle([rx0 + 18, 172, rx0 + 88, 196], radius=12, fill=(*C_GH_PURPLE, 255))
        draw_centered_text(d, rx0 + 53, 184, "Merged", FONT_SM_BOLD, C_GH_TEXT)
        d.text((rx0 + 98, 177), "merged 2 commits into main", font=FONT_SM, fill=C_GH_MUTED)
    else:
        d.rounded_rectangle([rx0 + 18, 172, rx0 + 76, 196], radius=12, fill=(*C_GH_GREEN_BTN, 255))
        draw_centered_text(d, rx0 + 47, 184, "Open", FONT_SM_BOLD, C_GH_TEXT)
        d.text((rx0 + 86, 177), "wants to merge 2 commits into main", font=FONT_SM, fill=C_GH_MUTED)

    # PR Tab Bar (evenly spaced with clear breathing room)
    tab_line_y = 240
    d.line([(rx0 + 1, tab_line_y), (rx1 - 1, tab_line_y)], fill=(*C_GH_BORDER, 255), width=1)
    pr_tabs = [
        ("Conversation", rx0 + 52, not state["checks_tab"]),
        ("Commits  2", rx0 + 128, False),
        ("Checks  3", int(round(P_CHECKS[0])), state["checks_tab"]),
    ]
    for tname, tcx, is_act in pr_tabs:
        draw_centered_text(d, tcx, P_CHECKS[1], tname, FONT_SM_BOLD if is_act else FONT_SM, C_GH_TEXT if is_act else C_GH_MUTED)
        if is_act:
            tw = d.textbbox((0, 0), tname, font=FONT_SM_BOLD)[2] + 10
            d.line([(tcx - tw / 2, tab_line_y - 1), (tcx + tw / 2, tab_line_y - 1)], fill=(*C_GH_ORANGE, 255), width=2)

    # --- GitHub Card (y = 252..432, zero row/button overlap) ---
    cx0, cy0, cx1, cy1 = rx0 + 18, 252, rx1 - 18, 432
    d.rounded_rectangle([cx0, cy0, cx1, cy1], radius=6, fill=(*C_GH_SURFACE, 255), outline=(*C_GH_BORDER, 255), width=1)

    if state["checks_tab"] and not state["merged"]:
        d.text((cx0 + 16, cy0 + 10), "All checks have passed", font=FONT_UI_BOLD, fill=C_GH_TEXT)
        d.text((cx0 + 16, cy0 + 26), "3 successful checks on commit f38b10d", font=FONT_SM, fill=C_GH_MUTED)
        d.line([(cx0 + 1, cy0 + 44), (cx1 - 1, cy0 + 44)], fill=(*C_GH_BORDER, 255), width=1)

        # 3 rows at y = 304, 325, 346 -> bottom at 358, 12px above divider at 370
        jy = cy0 + 52
        jobs = [("test (x86_64-linux)", "12s"), ("test (aarch64-macos)", "14s"), ("clippy / fmt", "4s")]
        for jname, jtime in jobs:
            ov.alpha_composite(icons["check_green"], (cx0 + 16, jy - 1))
            d.text((cx0 + 36, jy), jname, font=FONT_SM, fill=C_GH_TEXT)
            d.text((cx1 - 44, jy), jtime, font=FONT_MONO, fill=C_GH_MUTED)
            jy += 21
    else:
        if state["merged"]:
            ov.alpha_composite(icons["check_purple"], (cx0 + 16, cy0 + 18))
            d.text((cx0 + 38, cy0 + 12), "Pull request successfully merged", font=FONT_UI_BOLD, fill=C_GH_TEXT)
            d.text((cx0 + 38, cy0 + 29), "You're all set — fix/hidpi branch can be deleted", font=FONT_SM, fill=C_GH_MUTED)
        elif state["reloaded"]:
            ov.alpha_composite(icons["check_green"], (cx0 + 16, cy0 + 18))
            d.text((cx0 + 38, cy0 + 12), "All checks have passed", font=FONT_UI_BOLD, fill=C_GH_TEXT)
            d.text((cx0 + 38, cy0 + 29), "3 successful checks  •  No conflicts with main", font=FONT_SM, fill=C_GH_MUTED)
        else:
            d.ellipse([cx0 + 17, cy0 + 19, cx0 + 29, cy0 + 31], fill=(*C_GH_AMBER, 255))
            d.text((cx0 + 38, cy0 + 12), "Some checks haven't completed yet", font=FONT_UI_BOLD, fill=C_GH_TEXT)
            d.text((cx0 + 38, cy0 + 29), "2 in progress, 1 successful check", font=FONT_SM, fill=C_GH_MUTED)

        d.line([(cx0 + 1, cy0 + 52), (cx1 - 1, cy0 + 52)], fill=(*C_GH_BORDER, 255), width=1)
        d.text((cx0 + 16, cy0 + 66), "Branch: fix/hidpi  ->  main", font=FONT_MONO, fill=C_GH_MUTED)
        d.text((cx0 + 16, cy0 + 88), "+14  -4  in src/grid.rs", font=FONT_MONO, fill=C_GH_GREEN)

    # Bottom Merge Action Row (divider at 370, button centered at P_MERGE[1] = 396.7)
    d.line([(cx0 + 1, 370), (cx1 - 1, 370)], fill=(*C_GH_BORDER, 255), width=1)
    bx, by = P_MERGE[0], P_MERGE[1]
    if state["merged"]:
        d.rounded_rectangle([bx - 80, by - 15, bx + 80, by + 15], radius=6, fill=(*C_GH_PURPLE, 255))
        draw_centered_text(d, bx, by, "Merged into main", FONT_UI_BOLD, C_GH_TEXT)
    else:
        btn_col = C_GH_GREEN_BTN if state["reloaded"] else (35, 90, 48)
        d.rounded_rectangle([bx - 80, by - 15, bx + 80, by + 15], radius=6, fill=(*btn_col, 255))
        draw_centered_text(d, bx, by, "Merge pull request", FONT_UI_BOLD, C_GH_TEXT)

    frame.alpha_composite(ov)


# --- Power-User Macro Grid + On-Demand Subgrid Overlay ---
def composite_grid_and_subgrid(frame, alpha, filter_prefix=None, locked_macro=None, active_sub=None):
    if alpha <= 0.01:
        return
    ov = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")

    x0, y0, x1, y1 = 0.0, float(TOP_BAR_H), float(WIDTH), float(HEIGHT)

    scrim_a = 45 if locked_macro is None else 70
    d.rectangle([x0, y0, x1, y1], fill=(10, 12, 16, int(scrim_a * alpha)))

    line_a = 110 if locked_macro is None else 40
    line_col = (148, 163, 184, int(line_a * alpha))
    for c in range(1, GRID_COLS):
        gx = int(round(x0 + c * GW))
        d.line([(gx, y0), (gx, y1)], fill=line_col, width=1)
    for r in range(1, GRID_ROWS):
        gy = int(round(y0 + r * GH))
        d.line([(x0, gy), (x1, gy)], fill=line_col, width=1)

    if locked_macro is None:
        bw, bh = 40, 28
        for r in range(GRID_ROWS):
            for c in range(GRID_COLS):
                lbl = GRID_LABELS[r][c]
                matches = (filter_prefix is None) or lbl.startswith(filter_prefix)
                cell_a = alpha if matches else alpha * 0.22

                if filter_prefix is not None and matches:
                    ax0, ay0 = c * GW, TOP_BAR_H + r * GH
                    d.rectangle([ax0, ay0, ax0 + GW, ay0 + GH], fill=(*C_GRID_BLUE, int(26 * alpha)))

                cx, cy = (c + 0.5) * GW, TOP_BAR_H + (r + 0.5) * GH
                bg = (*C_GRID_BLUE, int(245 * cell_a)) if (filter_prefix and matches) else (*C_TERM_BAR, int(235 * cell_a))
                border = (*C_GH_TEXT, int(220 * cell_a)) if (filter_prefix and matches) else (*C_TERM_BORDER, int(220 * cell_a))

                d.rounded_rectangle([cx - bw / 2, cy - bh / 2, cx + bw / 2, cy + bh / 2], radius=6, fill=bg, outline=border, width=1)
                draw_centered_text(d, cx, cy, lbl, FONT_GRID, (*C_GH_TEXT, int(255 * cell_a)))
    else:
        mr, mc = locked_macro
        mx0, my0 = mc * GW, TOP_BAR_H + mr * GH
        mx1, my1 = mx0 + GW, my0 + GH

        d.rectangle([mx0, my0, mx1, my1], fill=(*C_GRID_BLUE, int(24 * alpha)))
        d.rectangle([mx0, my0, mx1, my1], outline=(*C_GRID_BLUE, int(245 * alpha)), width=2)

        sub_line = (*C_SUB_CYAN, int(170 * alpha))
        for sc in range(1, SUB_COLS):
            sx = int(round(mx0 + sc * SW))
            d.line([(sx, my0), (sx, my1)], fill=sub_line, width=1)
        for sr in range(1, SUB_ROWS):
            sy = int(round(my0 + sr * SH))
            d.line([(mx0, sy), (mx1, sy)], fill=sub_line, width=1)

        sbw, sbh = 24, 22
        for sr in range(SUB_ROWS):
            for sc in range(SUB_COLS):
                sx0, sy0 = mx0 + sc * SW, my0 + sr * SH
                scx, scy = sx0 + SW / 2.0, sy0 + SH / 2.0
                is_act = active_sub == (sr, sc)

                if is_act:
                    d.rectangle([sx0 + 1, sy0 + 1, sx0 + SW - 1, sy0 + SH - 1], fill=(*C_SUB_CYAN, int(70 * alpha)))
                    d.rectangle([sx0 + 1, sy0 + 1, sx0 + SW - 1, sy0 + SH - 1], outline=(*C_SUB_CYAN, int(255 * alpha)), width=2)

                bg = (*C_SUB_CYAN, int(255 * alpha)) if is_act else (*C_TERM_BG, int(238 * alpha))
                fg = (*C_GH_BG, int(255 * alpha)) if is_act else (*C_GH_TEXT, int(250 * alpha))
                border = (*C_GH_TEXT, int(255 * alpha)) if is_act else (*C_SUB_CYAN, int(200 * alpha))

                d.rounded_rectangle([scx - sbw / 2, scy - sbh / 2, scx + sbw / 2, scy + sbh / 2], radius=5, fill=bg, outline=border, width=1)
                draw_centered_text(d, scx, scy, SUB_LABELS[sr][sc], FONT_SUBGRID, fg)

    frame.alpha_composite(ov)


def composite_ripple(frame, x, y, progress, color=C_GH_BLUE):
    if progress <= 0.0 or progress >= 1.0:
        return
    ov = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")
    e = ease_out_cubic(progress)
    r = 4 + 24 * e
    d.ellipse([x - r, y - r, x + r, y + r], outline=(*color, int(245 * (1.0 - e))), width=2)
    frame.alpha_composite(ov)


def composite_hud(frame, mode_label, keys, accent=C_GH_BLUE):
    ov = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")

    key_ws = []
    for k, _ in keys:
        key_ws.append(max(30, int(d.textbbox((0, 0), k, font=FONT_KEY)[2] + 18)))

    mode_w, gap, pad_x = 58, 8, 12
    total_w = pad_x * 2 + mode_w + (gap + sum(key_ws) + gap * (len(keys) - 1) if keys else 0)
    pill_h = 36
    px = (WIDTH - total_w) // 2
    py = HEIGHT - 48

    d.rounded_rectangle([px, py, px + total_w, py + pill_h], radius=9, fill=(*C_TERM_BAR, 250), outline=(*C_TERM_BORDER, 255), width=1)

    mx0, my0 = px + pad_x, py + 7
    active = mode_label != "IDLE"
    d.rounded_rectangle([mx0, my0, mx0 + mode_w, my0 + 22], radius=5, fill=(*accent, 52) if active else (*C_TERM_DIM, 255))
    draw_centered_text(d, mx0 + mode_w / 2, my0 + 11, mode_label, FONT_BADGE, accent if active else C_GH_MUTED)

    kx = mx0 + mode_w + gap
    for i in range(len(keys)):
        k, is_on = keys[i]
        kw = key_ws[i]
        ky0, ky1 = py + 6, py + 30
        if is_on:
            d.rounded_rectangle([kx, ky0, kx + kw, ky1], radius=5, fill=(*accent, 255))
            fg = C_GH_BG if accent in (C_SUB_CYAN, C_GH_GREEN) else C_GH_TEXT
        else:
            d.rounded_rectangle([kx, ky0, kx + kw, ky1], radius=5, fill=(*C_TERM_DIM, 255), outline=(*C_TERM_BORDER, 255), width=1)
            fg = C_GH_TEXT
        draw_centered_text(d, kx + kw / 2, (ky0 + ky1) / 2, k, FONT_KEY, fg)
        kx += kw + gap

    frame.alpha_composite(ov)


# --- Sequence Builder ---
def generate_frames():
    desktop = create_desktop()
    cursor_sprite, hot = create_cursor_sprite()
    icons = {
        "reload": create_reload_sprite(C_CHR_ICON),
        "reload_active": create_reload_sprite(C_GH_TEXT),
        "back": create_back_sprite(C_GH_MUTED),
        "check_green": create_check_sprite(C_GH_GREEN),
        "check_purple": create_check_sprite(C_GH_PURPLE),
    }

    actions = [
        {"pos": P_RELOAD, "macro": (0, 2), "sub": (1, 0), "m_lbl": "AD", "s_lbl": "A", "act": "reload", "col": C_GH_GREEN, "tail": 10},
        {"pos": P_CHECKS, "macro": (1, 2), "sub": (0, 2), "m_lbl": "SD", "s_lbl": "E", "act": "checks", "col": C_GH_BLUE, "tail": 10},
        {"pos": P_MERGE, "macro": (2, 2), "sub": (0, 1), "m_lbl": "DD", "s_lbl": "W", "act": "merge", "col": C_GH_PURPLE, "tail": 10},
        {"pos": P_TERM, "macro": (1, 0), "sub": (1, 2), "m_lbl": "SA", "s_lbl": "D", "act": "term", "col": C_GH_BLUE, "tail": 6},
    ]

    state = {
        "focus_right": False,
        "reloaded": False,
        "reload_flash": False,
        "checks_tab": False,
        "merged": False,
        "typed_chars": 0,
        "term_executed": False,
    }
    cur = P_TERM
    frames_rgba = []

    # 1. Idle (12 frames = 0.48s)
    for _ in range(12):
        fr = desktop.copy()
        draw_split_workspace(fr, state, icons)
        composite_hud(fr, "IDLE", [("Caps", False)])
        fr.alpha_composite(cursor_sprite, (int(cur[0] - hot[0]), int(cur[1] - hot[1])))
        frames_rgba.append(fr)

    # 2. Four readable 3-keystroke Grid + Subgrid actions
    for act in actions:
        start, end = cur, act["pos"]
        m_lbl, s_lbl = act["m_lbl"], act["s_lbl"]
        total_steps = 28 + act["tail"]

        for s in range(total_steps):
            fr = desktop.copy()
            g_alpha = 0.0
            f_prefix = None
            l_macro = None
            a_sub = None
            rip = 0.0
            hud_accent = C_GH_BLUE
            state["reload_flash"] = False

            if s < 6:
                g_alpha = ease_out_cubic((s + 1) / 4.0)
                cx, cy = start
                mode, keys = "GRID", [("Caps", True)]
            elif s < 11:
                g_alpha = 1.0
                f_prefix = m_lbl[0]
                cx, cy = start
                mode, keys = "GRID", [("Caps", True), (m_lbl[0], True)]
            elif s < 19:
                g_alpha = 1.0
                l_macro = act["macro"]
                cx, cy = start
                mode, keys = "GRID", [("Caps", True), (m_lbl, True)]
            elif s < 24:
                g_alpha = 1.0
                l_macro = act["macro"]
                a_sub = act["sub"]
                cx, cy = start
                hud_accent = C_SUB_CYAN
                mode, keys = "SUB", [("Caps", True), (m_lbl, False), (s_lbl, True)]
            elif s < 28:
                t = (s - 23) / 4.0
                g_alpha = 1.0 - ease_out_cubic(t)
                l_macro = act["macro"]
                a_sub = act["sub"]
                e = ease_out_expo(t)
                cx, cy = lerp(start[0], end[0], e), lerp(start[1], end[1], e)
                hud_accent = C_SUB_CYAN
                mode, keys = "SUB", [(m_lbl, False), (s_lbl, s < 26)]
            else:
                cx, cy = end
                if act["act"] == "reload" and s < 33:
                    state["reload_flash"] = True
                if s == 29:
                    if act["act"] == "reload":
                        state["focus_right"] = True
                        state["reloaded"] = True
                    elif act["act"] == "checks":
                        state["focus_right"] = True
                        state["checks_tab"] = True
                    elif act["act"] == "merge":
                        state["focus_right"] = True
                        state["merged"] = True
                    elif act["act"] == "term":
                        state["focus_right"] = False
                rip = (s - 28) / 10.0
                hud_accent = act["col"]
                mode, keys = "CLICK", [("Space", s < 33)]

            draw_split_workspace(fr, state, icons)
            composite_grid_and_subgrid(fr, g_alpha, f_prefix, l_macro, a_sub)
            composite_ripple(fr, cx, cy, rip, act["col"])
            composite_hud(fr, mode, keys, hud_accent)
            fr.alpha_composite(cursor_sprite, (int(round(cx - hot[0])), int(round(cy - hot[1]))))
            frames_rgba.append(fr)

        cur = end

    # 3. Live typing in focused Left Terminal ("git pull origin main" + Enter)
    cmd_len = len(TYPED_CMD)
    for t_step in range(cmd_len + 14):
        fr = desktop.copy()
        if t_step < cmd_len:
            state["typed_chars"] = t_step + 1
            mode = "TYPE"
            hud_accent = C_GH_GREEN
            ch = TYPED_CMD[t_step]
            disp_key = "Space" if ch == " " else ch
            keys = [(disp_key, True)]
        elif t_step < cmd_len + 3:
            mode = "TYPE"
            hud_accent = C_GH_GREEN
            keys = [("Enter", False)]
        else:
            state["term_executed"] = True
            pressed_enter = t_step < cmd_len + 7
            mode = "TYPE" if pressed_enter else "IDLE"
            hud_accent = C_GH_GREEN if pressed_enter else C_GH_BLUE
            keys = [("Enter", True)] if pressed_enter else [("Caps", False)]

        draw_split_workspace(fr, state, icons)
        composite_hud(fr, mode, keys, hud_accent)
        fr.alpha_composite(cursor_sprite, (int(round(cur[0] - hot[0])), int(round(cur[1] - hot[1]))))
        frames_rgba.append(fr)

    # 4. Final hold + 10-frame crossfade cleanly back to frame 0
    for _ in range(18):
        fr = desktop.copy()
        draw_split_workspace(fr, state, icons)
        composite_hud(fr, "IDLE", [("Caps", False)])
        fr.alpha_composite(cursor_sprite, (int(round(cur[0] - hot[0])), int(round(cur[1] - hot[1]))))
        frames_rgba.append(fr)

    frames_rgb = []
    for fr in frames_rgba:
        frames_rgb.append(fr.convert("RGB"))

    fade_len = 10
    first_rgb = frames_rgb[0]
    for i in range(fade_len):
        idx = len(frames_rgb) - fade_len + i
        frames_rgb[idx] = Image.blend(frames_rgb[idx], first_rgb, ease_out_cubic((i + 1) / float(fade_len)))

    return frames_rgb


# --- Swatch-Pinned 256-Color Palette Encoder (Zero Color Loss) ---
def save_optimized_gif(frames_rgb, output_path, fps=25):
    n = len(frames_rgb)
    sample_indices = [0, 18, 28, 36, 74, 112, 150, n - 18]

    pinned_colors = [
        C_DESKTOP, C_MENUBAR, C_TERM_BG, C_TERM_BAR, C_TERM_BORDER, C_TERM_DIM,
        C_TL_RED, C_TL_YELLOW, C_TL_GREEN, C_TL_INACTIVE,
        C_CHR_STRIP, C_CHR_TOOLBAR, C_CHR_ICON,
        C_GH_BG, C_GH_SURFACE, C_GH_BORDER, C_GH_TEXT, C_GH_MUTED,
        C_GH_GREEN_BTN, C_GH_GREEN, C_GH_BLUE, C_GH_AMBER, C_GH_PURPLE, C_GH_CYAN,
        C_SUB_CYAN, C_GH_ORANGE,
    ]
    swatch_h = 64
    strip = Image.new("RGB", (WIDTH, HEIGHT * len(sample_indices) + swatch_h))
    for i in range(len(sample_indices)):
        idx = sample_indices[i]
        strip.paste(frames_rgb[min(idx, n - 1)], (0, i * HEIGHT))

    sw_draw = ImageDraw.Draw(strip)
    sw_y = HEIGHT * len(sample_indices)
    sw_w = WIDTH // len(pinned_colors)
    for i in range(len(pinned_colors)):
        sw_draw.rectangle([i * sw_w, sw_y, (i + 1) * sw_w, sw_y + swatch_h], fill=pinned_colors[i])

    master = strip.quantize(colors=256, method=QUANTIZE_MEDIANCUT, dither=DITHER_NONE)
    paletted = []
    for fr in frames_rgb:
        paletted.append(fr.quantize(palette=master, dither=DITHER_NONE))

    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    paletted[0].save(
        output_path,
        save_all=True,
        append_images=paletted[1:],
        duration=1000 // fps,
        loop=0,
        optimize=True,
        disposal=1,
    )
    print(f"Saved {output_path} ({WIDTH}x{HEIGHT}, {n} frames @ {fps} FPS, {os.path.getsize(output_path)/1024:.1f} KB)")


if __name__ == "__main__":
    save_optimized_gif(generate_frames(), OUT_PATH, fps=FPS)
