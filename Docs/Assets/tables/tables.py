#!/usr/bin/env python3
"""Draw the README's tables as SVG, light and dark, in each README language.

    python3 Docs/Assets/tables/tables.py

writes `<name>{-lang}-{light,dark}.svg` next to this script, Simplified
Chinese without a language suffix. The look, the framework marks and the
text measuring come from the comparison charts (`../compare/charts.py`);
platform marks are Simple Icons paths from its `icons.json` (CC0; the marks
belong to their owners). Inline `code` and **bold** work in every string.
"""

import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compare"))

from charts import FRAMEWORKS, ICONS, THEMES, badge, esc, svg, width  # noqa: E402

from text import ALT, TEXT  # noqa: E402

MONO = "ui-monospace, SFMono-Regular, Menlo, Consolas, 'Liberation Mono', monospace"
W = 960
LANGS = (("zh-Hans", ""), ("zh-Hant", "-zh-Hant"), ("en", "-en"), ("ja", "-ja"))

# Status colors: (ink, fill).
TONES = {
    "light": dict(green=("#1A7F37", "#DAFBE1"), amber=("#9A6700", "#FFF3C4"), red=("#CF222E", "#FFEBE9"),
                  gray=("#59636E", "#EFF2F5"), blue=("#0969DA", "#DDF4FF")),
    "dark": dict(green=("#3FB950", "#12301E"), amber=("#D29922", "#33270F"), red=("#FF7B72", "#3B1C21"),
                 gray=("#9198A1", "#21262D"), blue=("#58A6FF", "#12273F")),
}
CODE_INK = {"light": "#0550AE", "dark": "#79C0FF"}


# Text: wrapping with inline styles.

NO_START = set("，。、；：！？）」』】》…,.;:!?)")
NO_END = set("（「『【《“‘(")


def cjk(c):
    return ord(c) > 0x2E80


def parse(text):
    pieces = []
    for part in re.split(r"(`[^`]+`|\*\*[^*]+\*\*)", text):
        if part.startswith("`"):
            pieces.append((part[1:-1], "code"))
        elif part.startswith("**"):
            pieces.append((part[2:-2], "b"))
        elif part:
            pieces.append((part, None))
    return pieces


def measure(text, style, size):
    if style == "code":
        return sum(1.0 if cjk(c) else 0.6 for c in text) * size * 0.92
    return width(text, size) * (1.05 if style == "b" else 1.0)


def tokens(text):
    out = []
    for chunk, style in parse(text):
        for m in re.finditer(r"[ \t]+|[^ \t\u2e80-\uffff]+|[\u2e80-\uffff]", chunk):
            s = m.group()
            if s.isspace():
                out.append([(" ", None)])
                continue
            if out and not out[-1][-1][0].isspace():
                last = out[-1][-1][0][-1]
                if s[0] in NO_START or last in NO_END or not (cjk(last) or cjk(s[0])):
                    out[-1].append((s, style))
                    continue
            out.append([(s, style)])
    return out


def wrap(text, size, maxw):
    lines = []
    for paragraph in text.split("\n"):
        line, used = [], 0.0
        for token in tokens(paragraph):
            tw = sum(measure(s, st, size) for s, st in token)
            if token[0][0] == " ":
                if line:
                    line.append(token)
                    used += tw
                continue
            if line and used + tw > maxw:
                while line and line[-1][0][0] == " ":
                    line.pop()
                lines.append(line)
                line, used = [], 0.0
            line.append(token)
            used += tw
        while line and line[-1][0][0] == " ":
            line.pop()
        lines.append(line)
    return lines


def spans(line, theme, size):
    flat = []
    for token in line:
        for s, st in token:
            if flat and flat[-1][1] == st:
                flat[-1] = (flat[-1][0] + s, st)
            else:
                flat.append((s, st))
    out = []
    for s, st in flat:
        if st == "code":
            out.append(f'<tspan font-family="{MONO}" font-size="{size * 0.92:.1f}" fill="{CODE_INK[theme]}">{esc(s)}</tspan>')
        elif st == "b":
            out.append(f'<tspan font-weight="700">{esc(s)}</tspan>')
        else:
            out.append(esc(s))
    return "".join(out)


def para(text, x, top, size, maxw, fill, theme, lh=None, weight=None, anchor="start", family=None):
    """Wrapped text whose first line box starts at `top`; returns (parts, height)."""
    lh = lh or round(size * 1.5, 1)
    bold = weight is not None and int(weight) >= 600
    lines = wrap(text, size, maxw / (1.05 if bold else 1.0))
    attrs = (f' font-weight="{weight}"' if weight else "") + (f' text-anchor="{anchor}"' if anchor != "start" else "")
    attrs += f' font-family="{family}"' if family else ""
    out = []
    for i, line in enumerate(lines):
        base = top + i * lh + (lh - size) / 2 + size * 0.8
        out.append(f'<text x="{x:.1f}" y="{base:.1f}" font-size="{size}" fill="{fill}"{attrs}>{spans(line, theme, size)}</text>')
    return out, len(lines) * lh


def pill(x, y, label, tone, theme, size=12, h=22, mono=False, anchor="start"):
    """A rounded status label; returns (svg, width)."""
    ink, fill = TONES[theme][tone]
    tw = sum(1.0 if cjk(c) else 0.6 for c in label) * size if mono else width(label, size) * 1.05
    w = tw + 20
    if anchor == "end":
        x -= w
    family = f' font-family="{MONO}"' if mono else ""
    return (f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h}" rx="{h / 2}" fill="{fill}"/>'
            f'<text x="{x + w / 2:.1f}" y="{y + h / 2 + size * 0.36:.1f}" font-size="{size}" font-weight="600" '
            f'text-anchor="middle" fill="{ink}"{family}>{esc(label)}</text>'), w


def group(parts, x, y):
    return f'<g transform="translate({x:.1f} {y:.1f})">' + "".join(parts) + "</g>"


# Marks.

GLYPHS = {
    "window": '<rect x="3" y="4.5" width="18" height="15" rx="3"/><path d="M3 9.5h18"/>',
    "sparkle": '<path d="M11 3.5c.6 4.3 3.2 6.9 7.5 7.5-4.3.6-6.9 3.2-7.5 7.5-.6-4.3-3.2-6.9-7.5-7.5 4.3-.6 6.9-3.2 7.5-7.5z"/>'
               '<path d="M19 15.5v5M16.5 18h5"/>',
    "bolt": '<path d="M13.5 2.5 5 13.5h6.5l-1 8 8.5-11h-6.5z"/>',
    "terminal": '<rect x="3" y="4.5" width="18" height="15" rx="3"/><path d="m7.5 10 3 2.5-3 2.5M13 15h3.5"/>',
    "check": '<circle cx="12" cy="12" r="9"/><path d="m8 12.3 2.7 2.7 5.5-5.5"/>',
    "code": '<path d="M8.5 7 3.5 12l5 5M15.5 7l5 5-5 5"/>',
    "globe": '<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="3.8" ry="9"/><path d="M3 12h18"/>',
    "contrast": '<circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor" stroke="none"/>',
    "windows": '<rect x="3" y="8.5" width="13" height="11" rx="2.5"/>'
               '<path d="M8 8.5V7a2.5 2.5 0 0 1 2.5-2.5h8A2.5 2.5 0 0 1 21 7v6a2.5 2.5 0 0 1-2.5 2.5H16"/>',
    "arrow": '<path d="M4.5 12h15M13.5 6l6 6-6 6"/>',
    "tick": '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
}


def glyph(name, x, y, size, color, stroke=1.8):
    return (f'<g transform="translate({x:.1f} {y:.1f}) scale({size / 24:.4f})" fill="none" stroke="{color}" '
            f'stroke-width="{stroke}" stroke-linecap="round" stroke-linejoin="round" color="{color}">{GLYPHS[name]}</g>')


def icon_tile(name, x, y, theme, size=40):
    t = THEMES[theme]
    return (f'<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{size * 0.28:.1f}" fill="{t["soft"]}"/>'
            + glyph(name, x + size * 0.22, y + size * 0.22, size * 0.56, t["accent"]))


PLATFORMS = {
    "apple": dict(color="#0A84FF", icon="apple"),
    "windows": dict(color="#0078D4"),
    "linux": dict(color="#4A86CF", icon="linux"),
    "android": dict(color="#073042", icon="android", ink="#3DDC84"),
    "wasm": dict(color="#654FF0", icon="webassembly"),
    "globe": dict(color="#654FF0", glyph="globe"),
    "next": dict(glyph="arrow"),
}


def platform_tile(key, x, y, size, theme):
    t, p = THEMES[theme], PLATFORMS[key]
    rx = size * 0.26
    if "color" not in p:
        return (f'<rect x="{x + 0.5:.1f}" y="{y + 0.5:.1f}" width="{size - 1}" height="{size - 1}" rx="{rx:.1f}" '
                f'fill="{t["panel"]}" stroke="{t["border"]}"/>' + glyph(p["glyph"], x + size * 0.2, y + size * 0.2, size * 0.6, t["muted"], 2))
    out = [f'<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{rx:.1f}" fill="{p["color"]}"/>']
    inner = size * 0.58
    ox, oy = x + (size - inner) / 2, y + (size - inner) / 2
    if key == "windows":
        half, gap = inner / 2, inner * 0.06
        for dx in (0, half + gap / 2):
            for dy in (0, half + gap / 2):
                out.append(f'<rect x="{ox + dx:.2f}" y="{oy + dy:.2f}" width="{half - gap / 2:.2f}" height="{half - gap / 2:.2f}" fill="#FFFFFF"/>')
    elif "icon" in p:
        for d in ICONS[p["icon"]]["paths"]:
            out.append(f'<path d="{d}" fill="{p.get("ink", "#FFFFFF")}" transform="translate({ox:.2f} {oy:.2f}) scale({inner / 24:.4f})"/>')
    else:
        out.append(glyph(p["glyph"], ox, oy, inner, "#FFFFFF", 2))
    return "".join(out)


def card(x, y, w, h, theme, highlight=False):
    t = THEMES[theme]
    if highlight:
        return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="{t["soft"]}" stroke="{t["accent"]}" stroke-width="1.5"/>'
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="{t["panel"]}" stroke="{t["border"]}"/>'


def grid(cells, columns, theme, top=24, gap=16, margin=24, highlight=()):
    """Lay out (parts, height) renderers as equal-width cards; returns (parts, bottom)."""
    cw = (W - 2 * margin - (columns - 1) * gap) / columns
    rendered = [render(cw) for render in cells]
    out, y = [], top
    for start in range(0, len(rendered), columns):
        row = rendered[start:start + columns]
        h = max(height for _, height in row)
        for i, (parts, _) in enumerate(row):
            x = margin + i * (cw + gap)
            out.append(card(x, y, cw, h, theme, start + i in highlight))
            out.append(group(parts, x, y))
        y += h + gap
    return out, y - gap


# 1. Why Bern.

def why(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["why"]

    def render_card(c):
        def render(cw):
            pad, inner = 22, cw - 44
            out = [icon_tile(c["icon"], pad, pad, theme)]
            parts, h = para(c["title"], pad, pad + 54, 17.5, inner, t["text"], theme, lh=25, weight=700)
            out += parts
            y = pad + 54 + h + 10
            for kind, value in c["blocks"]:
                if kind == "lead":
                    parts, h = para(value, pad, y, 14.5, inner, t["text"], theme, lh=22)
                elif kind == "list":
                    parts, h = [], 0
                    for item in value:
                        parts.append(glyph("tick", pad - 1, y + h + 3.5, 15, t["accent"], 2.4))
                        p, ih = para(item, pad + 22, y + h, 14, inner - 22, t["text"], theme, lh=22)
                        parts += p
                        h += ih + 2
                elif kind == "tag":
                    p, _ = pill(pad, y, value, "blue", theme, size=12, h=22)
                    parts, h = [p], 22
                elif kind == "text":
                    parts, h = para(value, pad, y, 14, inner, t["text"], theme, lh=22)
                elif kind == "note":
                    parts, h = para(value, pad, y, 13, inner, t["muted"], theme, lh=19.5)
                elif kind == "stats":
                    head, rows = value
                    name_w = 74
                    col = (inner - name_w) / len(head)
                    parts = [f'<text x="{pad + name_w + i * col:.1f}" y="{y + 14}" font-size="12" fill="{t["muted"]}">{esc(label)}</text>'
                             for i, label in enumerate(head)]
                    h = 24
                    for name, values in rows:
                        parts.append(f'<line x1="{pad}" y1="{y + h:.1f}" x2="{pad + inner}" y2="{y + h:.1f}" stroke="{t["grid"]}"/>')
                        parts.append(f'<text x="{pad}" y="{y + h + 20:.1f}" font-size="13" fill="{t["muted"]}">{esc(name)}</text>')
                        for i, v in enumerate(values):
                            ink, weight = (t["muted"], 400) if v == "–" else (t["text"], 700)
                            parts.append(f'<text x="{pad + name_w + i * col:.1f}" y="{y + h + 20:.1f}" font-size="14.5" '
                                         f'font-weight="{weight}" fill="{ink}">{esc(v)}</text>')
                        h += 30
                out += parts
                y += h + 10
            return out, y + pad - 10
        return render

    body, bottom = grid([render_card(c) for c in s], 3, theme)
    return svg(W, bottom + 24, theme, "\n".join(body))


# 2. What the AI forgets.

def ai_checks(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["ai"]
    left, left_w, right, right_w = 32, 290, 376, 552
    out = [
        f'<text x="{left}" y="40" font-size="13" font-weight="600" fill="{t["muted"]}">{esc(s["head"][0])}</text>',
        f'<text x="{right}" y="40" font-size="13" font-weight="600" fill="{t["muted"]}">{esc(s["head"][1])}</text>',
    ]
    y = 56
    for i, (problem, code, explain) in enumerate(s["rows"]):
        out.append(f'<line x1="24" y1="{y}" x2="{W - 24}" y2="{y}" stroke="{t["border"] if i == 0 else t["grid"]}"/>')
        top = y + 18
        parts, ph = para(problem, left, top, 15, left_w, t["text"], theme, lh=23, weight=600)
        out += parts
        out.append(glyph("arrow", 336, top + 2.5, 18, t["line"], 2))
        if code:
            p, _ = pill(right, top, f"error[{code}]", "red", theme, size=12.5, h=23, mono=True)
        else:
            p, _ = pill(right, top, s["impossible"], "blue", theme, size=12.5, h=23)
        out.append(p)
        parts, eh = para(explain, right, top + 31, 14, right_w, t["muted"], theme, lh=22)
        out += parts
        y = top + max(ph, 31 + eh) + 18
    return svg(W, y + 6, theme, "\n".join(out))


# 3. Measured.

def measured(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["measured"]
    cw = (W - 48 - 32) / 3
    inner = cw - 44
    notes = [para(p["note"], 0, 0, 12.5, inner, t["muted"], theme, lh=19)[1] for p in s["platforms"]]
    note_h = max(notes)
    h = 290 + note_h + 20
    out = []
    for i, (key, p) in enumerate(zip(("apple", "windows", "wasm"), s["platforms"])):
        x, y = 24 + i * (cw + 16), 24
        out.append(card(x, y, cw, h, theme))
        g = [platform_tile(key, 22, 22, 34, theme),
             f'<text x="68" y="36" font-size="16.5" font-weight="700" fill="{t["text"]}">{esc(p["name"])}</text>',
             f'<text x="68" y="53" font-size="12.5" fill="{t["muted"]}">{esc(p["toolkit"])}</text>',
             f'<line x1="22" y1="74" x2="{cw - 22:.1f}" y2="74" stroke="{t["border"]}"/>']
        for row, (label, (value, unit)) in enumerate(zip(s["labels"], p["values"])):
            top = 92 + row * 66
            g.append(f'<text x="22" y="{top + 12}" font-size="12.5" fill="{t["muted"]}">{esc(label)}</text>')
            if value == "–":
                g.append(f'<text x="22" y="{top + 46}" font-size="28" fill="{t["line"]}">–</text>')
            else:
                g.append(f'<text x="22" y="{top + 46}" font-size="30" font-weight="700" fill="{t["text"]}">{esc(value)}'
                         f'<tspan font-size="15" font-weight="500" fill="{t["muted"]}" dx="5">{esc(unit)}</tspan></text>')
        parts, _ = para(p["note"], 22, 290, 12.5, inner, t["muted"], theme, lh=19)
        g += parts
        out.append(group(g, x, y))
    return svg(W, h + 48, theme, "\n".join(out))


# 4. More.

def more(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["more"]

    def render_card(icon, title, text):
        def render(cw):
            inner = cw - 44
            title_parts, th = para(title, 74, 0, 16.5, inner - 52, t["text"], theme, lh=23, weight=700)
            ty = 22 + (40 - th) / 2
            out = [icon_tile(icon, 22, 22, theme)] + [group(title_parts, 0, ty)]
            parts, h = para(text, 22, 78, 14, inner, t["muted"], theme, lh=22)
            return out + parts, 78 + h + 22
        return render

    icons = ("terminal", "check", "code", "globe", "contrast", "windows")
    body, bottom = grid([render_card(i, *c) for i, c in zip(icons, s)], 3, theme)
    return svg(W, bottom + 24, theme, "\n".join(body))


# 5. Platforms.

def platforms(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["platforms"]
    cols = (32, 262, 650, 824)
    out = [f'<text x="{x}" y="40" font-size="13" font-weight="600" fill="{t["muted"]}">{esc(label)}</text>'
           for x, label in zip(cols, s["head"])]
    tones = dict(ok="green", sim="amber", plan="gray")
    y = 56
    for i, (key, name, code, ui, req, engine, status) in enumerate(s["rows"]):
        out.append(f'<line x1="24" y1="{y}" x2="{W - 24}" y2="{y}" stroke="{t["border"] if i == 0 else t["grid"]}"/>')
        top = y + 16
        out.append(platform_tile(key, cols[0], top, 34, theme))
        name_top = top + (0 if code else 8)
        parts, _ = para(name, cols[0] + 46, name_top, 15, 150, t["text"], theme, lh=20, weight=700)
        out += parts
        if code:
            out.append(f'<text x="{cols[0] + 46}" y="{top + 35}" font-size="12" font-family="{MONO}" fill="{t["muted"]}">{esc(code)}</text>')
        parts, uh = para(ui, cols[1], top, 14, 364, t["text"], theme, lh=21)
        out += parts
        if req:
            parts, rh = para(req, cols[1], top + uh + 3, 12.5, 364, t["muted"], theme, lh=19)
            out += parts
            uh += rh + 3
        parts, eh = para(engine, cols[2], top, 13.5, 160, t["text"] if engine != "–" else t["line"], theme, lh=21)
        out += parts
        p, _ = pill(cols[3], top + 4, s["status"][status], tones[status], theme)
        out.append(p)
        y = top + max(38, uh, eh) + 16
    return svg(W, y + 6, theme, "\n".join(out))


# 6. Technology.

def technology(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["technology"]
    cols = ((28, 128), (162, 130), (298, 116), (420, 236), (662, 154), (822, 114))
    out = [para(label, x, 26, 12.5, w, t["muted"], theme, lh=18, weight=600)[0][0] for (x, w), label in zip(cols, s["head"])]
    y = 58
    out.append(f'<line x1="20" y1="{y}" x2="{W - 20}" y2="{y}" stroke="{t["border"]}"/>')
    for i, (key, cells) in enumerate(s["rows"]):
        top = y + 14
        rendered = [para(cell, x, top, 13.5, w, t["text"] if c < 4 else t["muted"], theme, lh=20)
                    for c, (cell, (x, w)) in enumerate(zip(cells, cols[1:]))]
        h = max(max(rh for _, rh in rendered), 28)
        bottom = top + h + 14
        if key == "bern":
            out.append(f'<rect x="16" y="{y + 4}" width="{W - 32}" height="{bottom - y - 8}" rx="12" fill="{t["soft"]}"/>')
            out.append(f'<rect x="16" y="{y + 4}" width="4" height="{bottom - y - 8}" rx="2" fill="{t["accent"]}"/>')
        elif i > 0:
            out.append(f'<line x1="20" y1="{y}" x2="{W - 20}" y2="{y}" stroke="{t["grid"]}"/>')
        out.append(badge(key, cols[0][0] + 12, top + 11, 12))
        out.append(f'<text x="{cols[0][0] + 32}" y="{top + 16}" font-size="14.5" font-weight="700" fill="{t["text"]}">{esc(FRAMEWORKS[key]["name"])}</text>')
        for parts, _ in rendered:
            out += parts
        y = bottom
    return svg(W, y + 10, theme, "\n".join(out))


# 7. Platforms and capabilities.

MATRIX = [
    ("bern", "FFFPFFFF"), ("gpui", "FFFNNNNN"), ("dioxus", "FFFFFNNN"), ("flutter", "FFFFFNNN"),
    ("rn", "PPNFPFNN"), ("uniapp", "NNNFFPNN"), ("electron", "FFFNNNNN"), ("tauri", "FFFFNNNN"),
    ("avalonia", "FFFFFNNN"), ("qt", "FFFFFPNN"),
]


def mark(kind, cx, cy, theme, r=7):
    t = THEMES[theme]
    if kind == "F":
        return f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r}" fill="{t["accent"]}"/>'
    if kind == "P":
        return (f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r - 0.8}" fill="none" stroke="{t["accent"]}" stroke-width="1.6"/>'
                f'<path d="M{cx:.1f} {cy - r + 0.8:.1f} A{r - 0.8} {r - 0.8} 0 0 0 {cx:.1f} {cy + r - 0.8:.1f} Z" fill="{t["accent"]}"/>')
    return f'<line x1="{cx - 6:.1f}" y1="{cy:.1f}" x2="{cx + 6:.1f}" y2="{cy:.1f}" stroke="{t["line"]}" stroke-width="2" stroke-linecap="round"/>'


def capabilities(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["capabilities"]
    x0, cw = 196, (W - 24 - 196) / 8
    out = []
    # Legend.
    lx = 28
    for kind, label in zip("FPN", s["legend"]):
        out.append(mark(kind, lx + 7, 30, theme))
        out.append(f'<text x="{lx + 22}" y="35" font-size="13" fill="{t["muted"]}">{esc(label)}</text>')
        lx += 22 + width(label, 13) + 26
    # Groups and columns.
    for start, span, label in ((0, 5, s["groups"][0]), (5, 3, s["groups"][1])):
        gx = x0 + start * cw
        out.append(f'<text x="{gx + span * cw / 2:.1f}" y="76" font-size="13" font-weight="700" text-anchor="middle" fill="{t["text"]}">{esc(label)}</text>')
        out.append(f'<path d="M{gx + 8:.1f} 92 v-5 h{span * cw - 16:.1f} v5" fill="none" stroke="{t["line"]}" stroke-width="1.2"/>')
    head_h = 0
    for i, label in enumerate(s["columns"]):
        parts, h = para(label, x0 + i * cw + cw / 2, 100, 12.5, cw - 10, t["muted"], theme, lh=17, weight=600, anchor="middle")
        out += parts
        head_h = max(head_h, h)
    out.append(f'<text x="28" y="{100 + head_h - 5:.1f}" font-size="12.5" font-weight="600" fill="{t["muted"]}">{esc(s["name"])}</text>')
    y = 100 + head_h + 10
    out.append(f'<line x1="20" y1="{y}" x2="{W - 20}" y2="{y}" stroke="{t["border"]}"/>')
    rows_top = y
    for i, (key, marks) in enumerate(MATRIX):
        top = y + 4
        if key == "bern":
            out.append(f'<rect x="16" y="{top}" width="{W - 32}" height="38" rx="10" fill="{t["soft"]}"/>')
            out.append(f'<rect x="16" y="{top}" width="4" height="38" rx="2" fill="{t["accent"]}"/>')
        elif i > 1:
            out.append(f'<line x1="20" y1="{y}" x2="{W - 20}" y2="{y}" stroke="{t["grid"]}"/>')
        cy = top + 19
        out.append(badge(key, 40, cy, 11))
        out.append(f'<text x="60" y="{cy + 5}" font-size="14.5" font-weight="700" fill="{t["text"]}">{esc(FRAMEWORKS[key]["name"])}</text>')
        for c, kind in enumerate(marks):
            out.append(mark(kind, x0 + c * cw + cw / 2, cy, theme))
        y += 46
    out.insert(0, f'<line x1="{x0 + 5 * cw:.1f}" y1="{rows_top + 6}" x2="{x0 + 5 * cw:.1f}" y2="{y - 6}" stroke="{t["grid"]}" stroke-width="1.5"/>')
    return svg(W, y + 16, theme, "\n".join(out))


# 8. Which one to choose.

def choose(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["choose"]
    good, cost, not_yet = s["labels"]
    green, amber = TONES[theme]["green"][0], TONES[theme]["amber"][0]

    def section(label, ink, text, x, y, w, fill):
        parts = [f'<text x="{x}" y="{y + 13}" font-size="12.5" font-weight="700" fill="{ink}">{esc(label)}</text>']
        p, h = para(text, x, y + 21, 13.5, w, fill, theme, lh=21)
        return parts + p, 21 + h

    def render_card(key, fit, price):
        def render(cw):
            inner = cw - 40
            out = [badge(key, 20 + 15, 20 + 15, 15),
                   f'<text x="62" y="41" font-size="16.5" font-weight="700" fill="{t["text"]}">{esc(FRAMEWORKS[key]["name"])}</text>']
            parts, h1 = section(good, green, fit, 20, 66, inner, t["text"])
            parts2, h2 = section(cost, amber, price, 20, 66 + h1 + 12, inner, t["muted"])
            return out + parts + parts2, 66 + h1 + 12 + h2 + 20
        return render

    body, bottom = grid([render_card(*c) for c in s["rows"]], 3, theme)
    # Bern, across the width.
    y, x, w = bottom + 16, 24, W - 48
    half = (w - 40 - 32) / 2
    parts, h1 = section(good, green, s["bern"][0], 20, 72, half, t["text"])
    parts2, h2 = section(not_yet, amber, s["bern"][1], 20 + half + 32, 72, half, t["muted"])
    h = 72 + max(h1, h2) + 22
    body.append(card(x, y, w, h, theme, highlight=True))
    body.append(group([badge("bern", 20 + 18, 20 + 18, 18),
                       f'<text x="68" y="45" font-size="19" font-weight="700" fill="{t["text"]}">Bern</text>'] + parts + parts2, x, y))
    return svg(W, y + h + 24, theme, "\n".join(body))


# 9. Roadmap.

def chip(x, y, label, kind, theme, suffix):
    t = THEMES[theme]
    size, h = 12.5, 24
    text = label + (f" · {suffix}" if suffix else "")
    tw = sum(measure(p, st, size) for p, st in parse(text))
    lead = 16 if kind == "done" else 0
    w = tw + 20 + lead
    if kind == "done":
        ink, fill = TONES[theme]["green"]
        shape = f'<rect x="{x}" y="{y}" width="{w:.1f}" height="{h}" rx="{h / 2}" fill="{fill}"/>' + glyph("tick", x + 8, y + 5.5, 13, ink, 2.6)
    elif kind == "next":
        ink, fill = TONES[theme]["blue"]
        shape = f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1:.1f}" height="{h - 1}" rx="{h / 2}" fill="{fill}" stroke="{ink}"/>'
    else:
        ink = t["muted"]
        shape = f'<rect x="{x + 0.5}" y="{y + 0.5}" width="{w - 1:.1f}" height="{h - 1}" rx="{h / 2}" fill="none" stroke="{t["line"]}" stroke-dasharray="3 3"/>'
    line = [[(p, st)] for p, st in parse(text)]
    label_svg = (f'<text x="{x + 10 + lead:.1f}" y="{y + h / 2 + size * 0.36:.1f}" font-size="{size}" font-weight="500" fill="{ink}">'
                 f'{spans(line, theme, size)}</text>')
    # Code inside a chip keeps the chip's color.
    return shape + label_svg.replace(f'fill="{CODE_INK[theme]}"', f'fill="{ink}"'), w


def roadmap(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["roadmap"]
    node_x, content_x, content_w = 104, 134, 640
    out, nodes = [], []
    y = 24
    for stage, text, chips, status in s["stages"]:
        top = y + 14
        out.append(f'<text x="78" y="{top + 17}" font-size="15" font-weight="700" text-anchor="end" fill="{t["text"]}">{esc(stage)}</text>')
        nodes.append((top + 11, status))
        parts, h = para(text, content_x, top, 14.5, content_w, t["text"], theme, lh=22)
        out += parts
        if chips:
            cx, cy = content_x, top + h + 10
            for label, kind in chips:
                suffix = {"next": s["next"], "todo": s["todo"]}.get(kind)
                shape, w = chip(0, 0, label, kind, theme, suffix)
                if cx + w > content_x + content_w:
                    cx, cy = content_x, cy + 32
                out.append(group([shape], cx, cy))
                cx += w + 8
            h = cy + 24 - top
        tone = "green" if status == "done" else "amber"
        p, _ = pill(W - 28, top, s["status"][status], tone, theme, anchor="end")
        out.append(p)
        y = top + max(h, 24) + 14
        out.append(f'<line x1="{content_x}" y1="{y}" x2="{W - 28}" y2="{y}" stroke="{t["grid"]}"/>')
    out.pop()
    line = [f'<line x1="{node_x}" y1="{nodes[0][0]}" x2="{node_x}" y2="{nodes[-1][0]}" stroke="{t["border"]}" stroke-width="2"/>']
    for cy, status in nodes:
        if status == "done":
            ink = TONES[theme]["green"][0]
            line.append(f'<circle cx="{node_x}" cy="{cy}" r="11" fill="{ink}"/>' + glyph("tick", node_x - 7, cy - 7, 14, t["bg"], 3))
        else:
            ink = TONES[theme]["amber"][0]
            line.append(f'<circle cx="{node_x}" cy="{cy}" r="10" fill="{t["bg"]}" stroke="{ink}" stroke-width="2.5"/>'
                        f'<circle cx="{node_x}" cy="{cy}" r="4" fill="{ink}"/>')
    return svg(W, y + 14, theme, "\n".join(line + out))


# 10. Repository.

def repository(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["repository"]
    label_x, path_x, path_w, desc_x, desc_w = 32, 168, 360, 548, 384
    out, y = [], 16
    for gi, (label, rows) in enumerate(s):
        if gi:
            out.append(f'<line x1="24" y1="{y}" x2="{W - 24}" y2="{y}" stroke="{t["border"]}"/>')
        y += 12
        parts, _ = para(label, label_x, y + 6, 13, path_x - label_x - 16, t["accent"], theme, lh=20, weight=700)
        out += parts
        for path, desc in rows:
            top = y + 6
            p1, h1 = para(path, path_x, top, 13.5, path_w, t["text"], theme, lh=20)
            p2, h2 = para(desc, desc_x, top, 13.5, desc_w, t["muted"], theme, lh=20)
            out += p1 + p2
            y = top + max(h1, h2) + 6
        y += 12
    return svg(W, y + 6, theme, "\n".join(out))


CHARTS = dict(why=why, ai=ai_checks, measured=measured, more=more, platforms=platforms, technology=technology,
              capabilities=capabilities, choose=choose, roadmap=roadmap, repository=repository)

if __name__ == "__main__":
    for lang, suffix in LANGS:
        for theme in THEMES:
            for name, draw in CHARTS.items():
                (HERE / f"{name}{suffix}-{theme}.svg").write_text(draw(theme, lang))
