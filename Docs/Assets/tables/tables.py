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

from charts import FONT, FRAMEWORKS, ICONS, THEMES, badge, bern_symbol, esc, width  # noqa: E402

from text import ALT, TEXT  # noqa: E402

MONO = "ui-monospace, SFMono-Regular, Menlo, Consolas, 'Liberation Mono', monospace"
W = 960
LANGS = (("zh-Hans", ""), ("zh-Hant", "-zh-Hant"), ("en", "-en"), ("ja", "-ja"))

# Status colors.
TONES = {
    "light": dict(green="#1A7F37", amber="#9A6700", red="#CF222E", gray="#59636E"),
    "dark": dict(green="#3FB950", amber="#D29922", red="#FF7B72", gray="#9198A1"),
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


def icon(name, x, y, theme, size=28):
    return glyph(name, x, y, size, THEMES[theme]["accent"], 1.9)


# Platform marks in their own colors, light and dark.
PLATFORMS = {
    "apple": dict(icon="apple"),
    "windows": dict(color=("#0078D4", "#3A96DD")),
    "linux": dict(icon="linux"),
    "android": dict(icon="android", color=("#2DA866", "#3DDC84")),
    "wasm": dict(icon="webassembly", color=("#654FF0", "#8F7FF7")),
    "globe": dict(glyph="globe", color=("#654FF0", "#8F7FF7")),
    "next": dict(glyph="arrow"),
}


def platform_mark(key, x, y, size, theme):
    t, p = THEMES[theme], PLATFORMS[key]
    color = p["color"][theme == "dark"] if "color" in p else (t["muted"] if key == "next" else t["text"])
    if key == "windows":
        half, gap = size / 2, size * 0.08
        return "".join(f'<rect x="{x + dx:.2f}" y="{y + dy:.2f}" width="{half - gap / 2:.2f}" height="{half - gap / 2:.2f}" fill="{color}"/>'
                       for dx in (0, half + gap / 2) for dy in (0, half + gap / 2))
    if "icon" in p:
        return "".join(f'<path d="{d}" fill="{color}" transform="translate({x:.2f} {y:.2f}) scale({size / 24:.4f})"/>'
                       for d in ICONS[p["icon"]]["paths"])
    return glyph(p["glyph"], x, y, size, color, 2)


def page(height, theme, body):
    """The SVG itself: no frame and no background, so it sits on the page."""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {height:.0f}" width="{W}" height="{height:.0f}" '
            f'font-family="{FONT}">\n<defs>{bern_symbol()}</defs>\n{body}\n</svg>\n')


def rule(y, theme, strong=False, x1=0, x2=W):
    t = THEMES[theme]
    return f'<line x1="{x1}" y1="{y:.1f}" x2="{x2}" y2="{y:.1f}" stroke="{t["border"] if strong else t["grid"]}"/>'


def label(x, y, text, ink, size=12.5, anchor="start"):
    """A colored word, standing on its own."""
    end = f' text-anchor="{anchor}"' if anchor != "start" else ""
    return f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="700" fill="{ink}"{end}>{esc(text)}</text>'


def columns(cells, count, gap=48, row_gap=40):
    """Lay out (parts, height) renderers in columns with only space between."""
    cw = (W - (count - 1) * gap) / count
    rendered = [render(cw) for render in cells]
    out, y = [], 0
    for start in range(0, len(rendered), count):
        row = rendered[start:start + count]
        for i, (parts, _) in enumerate(row):
            out.append(group(parts, i * (cw + gap), y))
        y += max(h for _, h in row) + row_gap
    return out, y - row_gap


# 1. Why Bern.

def why(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["why"]

    def render_column(c):
        def render(cw):
            out = [icon(c["icon"], 1, 1, theme)]
            parts, h = para(c["title"], 0, 42, 18, cw, t["text"], theme, lh=26, weight=700)
            out += parts
            y = 42 + h + 10
            for kind, value in c["blocks"]:
                if kind == "lead":
                    parts, h = para(value, 0, y, 14.5, cw, t["text"], theme, lh=22)
                elif kind == "list":
                    parts, h = [], 0
                    for item in value:
                        parts.append(glyph("tick", 0, y + h + 3.5, 15, t["accent"], 2.4))
                        p, ih = para(item, 22, y + h, 14, cw - 22, t["text"], theme, lh=22)
                        parts += p
                        h += ih + 2
                elif kind == "tag":
                    parts, h = [label(0, y + 13, value, t["accent"])], 12
                elif kind == "text":
                    parts, h = para(value, 0, y, 14, cw, t["text"], theme, lh=22)
                elif kind == "note":
                    parts, h = para(value, 0, y, 13, cw, t["muted"], theme, lh=19.5)
                elif kind == "stats":
                    head, rows = value
                    name_w = 74
                    col = (cw - name_w) / len(head)
                    parts = [f'<text x="{name_w + i * col:.1f}" y="{y + 14}" font-size="12" fill="{t["muted"]}">{esc(name)}</text>'
                             for i, name in enumerate(head)]
                    h = 24
                    for name, values in rows:
                        parts.append(rule(y + h, theme, x1=0, x2=cw))
                        parts.append(f'<text x="0" y="{y + h + 20:.1f}" font-size="13" fill="{t["muted"]}">{esc(name)}</text>')
                        for i, v in enumerate(values):
                            ink, weight = (t["line"], 400) if v == "–" else (t["text"], 700)
                            parts.append(f'<text x="{name_w + i * col:.1f}" y="{y + h + 20:.1f}" font-size="14.5" '
                                         f'font-weight="{weight}" fill="{ink}">{esc(v)}</text>')
                        h += 30
                out += parts
                y += h + 10
            return out, y - 10
        return render

    body, bottom = columns([render_column(c) for c in s], 3)
    return page(bottom + 4, theme, "\n".join(body))


# 2. What the AI forgets.

def ai_checks(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["ai"]
    left_w, right, right_w = 318, 364, 596
    red = TONES[theme]["red"]
    out = [label(0, 16, s["head"][0], t["muted"], 13), label(right, 16, s["head"][1], t["muted"], 13)]
    y = 30
    for i, (problem, code, explain) in enumerate(s["rows"]):
        out.append(rule(y, theme, strong=i == 0))
        top = y + 18
        parts, ph = para(problem, 0, top, 15, left_w, t["text"], theme, lh=23, weight=600)
        out += parts
        out.append(glyph("arrow", 330, top + 2.5, 18, t["line"], 2))
        if code:
            out.append(f'<text x="{right}" y="{top + 17}" font-size="13.5" font-weight="700" font-family="{MONO}" fill="{red}">error[{code}]</text>')
        else:
            out.append(label(right, top + 17, s["impossible"], t["accent"], 14))
        parts, eh = para(explain, right, top + 28, 14, right_w, t["muted"], theme, lh=22)
        out += parts
        y = top + max(ph, 28 + eh) + 18
    return page(y, theme, "\n".join(out))


# 3. Measured.

def measured(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["measured"]

    def render_column(key, p):
        def render(cw):
            g = [platform_mark(key, 1, 3, 26, theme),
                 f'<text x="38" y="17" font-size="17" font-weight="700" fill="{t["text"]}">{esc(p["name"])}</text>',
                 f'<text x="38" y="35" font-size="12.5" fill="{t["muted"]}">{esc(p["toolkit"])}</text>']
            for row, (name, (value, unit)) in enumerate(zip(s["labels"], p["values"])):
                top = 62 + row * 68
                g.append(f'<text x="0" y="{top + 12}" font-size="12.5" fill="{t["muted"]}">{esc(name)}</text>')
                if value == "–":
                    g.append(f'<text x="0" y="{top + 46}" font-size="28" fill="{t["line"]}">–</text>')
                else:
                    g.append(f'<text x="0" y="{top + 46}" font-size="30" font-weight="700" fill="{t["text"]}">{esc(value)}'
                             f'<tspan font-size="15" font-weight="500" fill="{t["muted"]}" dx="5">{esc(unit)}</tspan></text>')
            parts, h = para(p["note"], 0, 258, 12.5, cw, t["muted"], theme, lh=19)
            return g + parts, 258 + h
        return render

    body, bottom = columns([render_column(k, p) for k, p in zip(("apple", "windows", "wasm"), s["platforms"])], 3)
    return page(bottom + 4, theme, "\n".join(body))


# 4. More.

def more(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["more"]

    def render_cell(name, title, text):
        def render(cw):
            parts, th = para(title, 34, 0, 17, cw - 34, t["text"], theme, lh=26, weight=700)
            out = [icon(name, 1, 2, theme, size=22)] + parts
            body, h = para(text, 0, th + 8, 14, cw, t["muted"], theme, lh=22)
            return out + body, th + 8 + h
        return render

    icons = ("terminal", "check", "code", "globe", "contrast", "windows")
    body, bottom = columns([render_cell(i, *c) for i, c in zip(icons, s)], 3)
    return page(bottom + 4, theme, "\n".join(body))


# 5. Platforms.

def platforms(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["platforms"]
    cols = (0, 232, 636, 830)
    out = [label(x, 16, name, t["muted"], 13) for x, name in zip(cols, s["head"])]
    tones = dict(ok="green", sim="amber", plan="gray")
    y = 30
    for i, (key, name, code, ui, req, engine, status) in enumerate(s["rows"]):
        out.append(rule(y, theme, strong=i == 0))
        top = y + 16
        out.append(platform_mark(key, 1, top + 3, 26, theme))
        parts, _ = para(name, 40, top + (0 if code else 4), 15, 180, t["text"], theme, lh=20, weight=700)
        out += parts
        if code:
            out.append(f'<text x="40" y="{top + 36}" font-size="12" font-family="{MONO}" fill="{t["muted"]}">{esc(code)}</text>')
        parts, uh = para(ui, cols[1], top, 14, 380, t["text"], theme, lh=21)
        out += parts
        if req:
            parts, rh = para(req, cols[1], top + uh + 3, 12.5, 380, t["muted"], theme, lh=19)
            out += parts
            uh += rh + 3
        parts, eh = para(engine, cols[2], top, 13.5, 174, t["text"] if engine != "–" else t["line"], theme, lh=21)
        out += parts
        ink = TONES[theme][tones[status]]
        out.append(f'<circle cx="{cols[3] + 4}" cy="{top + 10.5}" r="4" fill="{ink}"/>' + label(cols[3] + 14, top + 15, s["status"][status], ink, 13.5))
        y = top + max(40, uh, eh) + 16
    return page(y, theme, "\n".join(out))


# 6. Technology.

def framework_name(key, x, cy, theme, r=12):
    t = THEMES[theme]
    ink = t["accent"] if key == "bern" else t["text"]
    return (badge(key, x + r, cy, r) +
            f'<text x="{x + 2 * r + 8}" y="{cy + 5}" font-size="14.5" font-weight="700" fill="{ink}">{esc(FRAMEWORKS[key]["name"])}</text>')


def technology(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["technology"]
    cols = ((0, 140), (148, 132), (290, 118), (418, 244), (672, 160), (842, 118))
    out = [para(name, x, 2, 12.5, w, t["muted"], theme, lh=18, weight=600)[0][0] for (x, w), name in zip(cols, s["head"])]
    y = 30
    for i, (key, cells) in enumerate(s["rows"]):
        out.append(rule(y, theme, strong=i == 0))
        top = y + 14
        rendered = [para(cell, x, top, 13.5, w, t["text"] if c < 4 else t["muted"], theme, lh=20)
                    for c, (cell, (x, w)) in enumerate(zip(cells, cols[1:]))]
        out.append(framework_name(key, 0, top + 10, theme))
        for parts, _ in rendered:
            out += parts
        y = top + max(max(h for _, h in rendered), 26) + 14
    return page(y, theme, "\n".join(out))


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
    x0 = 176
    cw = (W - x0) / 8
    out = []
    lx = 0
    for kind, name in zip("FPN", s["legend"]):
        out.append(mark(kind, lx + 7, 10, theme))
        out.append(f'<text x="{lx + 22}" y="15" font-size="13" fill="{t["muted"]}">{esc(name)}</text>')
        lx += 22 + width(name, 13) + 26
    for start, span, name in ((0, 5, s["groups"][0]), (5, 3, s["groups"][1])):
        gx = x0 + start * cw
        out.append(label(gx + span * cw / 2, 56, name, t["text"], 13, anchor="middle"))
        out.append(f'<path d="M{gx + 8:.1f} 72 v-5 h{span * cw - 16:.1f} v5" fill="none" stroke="{t["line"]}" stroke-width="1.2"/>')
    head_h = 0
    for i, name in enumerate(s["columns"]):
        parts, h = para(name, x0 + i * cw + cw / 2, 80, 12.5, cw - 10, t["muted"], theme, lh=17, weight=600, anchor="middle")
        out += parts
        head_h = max(head_h, h)
    out.append(label(0, 80 + head_h - 5, s["name"], t["muted"]))
    y = 80 + head_h + 10
    rows_top = y
    for i, (key, marks) in enumerate(MATRIX):
        out.append(rule(y, theme, strong=i == 0))
        cy = y + 23
        out.append(framework_name(key, 0, cy, theme, r=11))
        for c, kind in enumerate(marks):
            out.append(mark(kind, x0 + c * cw + cw / 2, cy, theme))
        y += 46
    out.append(f'<line x1="{x0 + 5 * cw:.1f}" y1="{rows_top + 8}" x2="{x0 + 5 * cw:.1f}" y2="{y - 8}" stroke="{t["grid"]}" stroke-width="1.5"/>')
    return page(y, theme, "\n".join(out))


# 8. Which one to choose.

def choose(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["choose"]
    good, cost, not_yet = s["labels"]
    green, amber = TONES[theme]["green"], TONES[theme]["amber"]

    def section(name, ink, text, x, y, w, fill):
        p, h = para(text, x, y + 20, 13.5, w, fill, theme, lh=21)
        return [label(x, y + 13, name, ink)] + p, 20 + h

    def render_cell(key, fit, price):
        def render(cw):
            out = [framework_name(key, 0, 14, theme, r=14)]
            parts, h1 = section(good, green, fit, 0, 42, cw, t["text"])
            parts2, h2 = section(cost, amber, price, 0, 42 + h1 + 12, cw, t["muted"])
            return out + parts + parts2, 42 + h1 + 12 + h2
        return render

    body, bottom = columns([render_cell(*c) for c in s["rows"]], 3)
    # Bern, across the width, under a rule.
    y = bottom + 32
    body.append(rule(y, theme, strong=True))
    half = (W - 48) / 2
    top = y + 28
    parts, h1 = section(good, green, s["bern"][0], 0, top + 42, half, t["text"])
    parts2, h2 = section(not_yet, amber, s["bern"][1], half + 48, top + 42, half, t["muted"])
    body += [framework_name("bern", 0, top + 16, theme, r=16)] + parts + parts2
    return page(top + 42 + max(h1, h2) + 4, theme, "\n".join(body))


# 9. Roadmap.

def item(x, y, text, kind, theme, suffix):
    """One piece of a stage: a mark and its words, no box."""
    t = THEMES[theme]
    size = 13
    words = text + (f" · {suffix}" if suffix else "")
    ink = {"done": TONES[theme]["green"], "next": t["accent"]}.get(kind, t["muted"])
    if kind == "done":
        mark_svg = glyph("tick", x, y - 11, 14, ink, 2.6)
    elif kind == "next":
        mark_svg = glyph("arrow", x, y - 11, 14, ink, 2.4)
    else:
        mark_svg = f'<circle cx="{x + 7}" cy="{y - 4.5}" r="4.5" fill="none" stroke="{ink}" stroke-width="1.5"/>'
    line = [[(p, st)] for p, st in parse(words)]
    words_svg = f'<text x="{x + 20}" y="{y}" font-size="{size}" font-weight="500" fill="{ink}">{spans(line, theme, size)}</text>'
    w = 20 + sum(measure(p, st, size) for p, st in parse(words))
    return mark_svg + words_svg.replace(f'fill="{CODE_INK[theme]}"', f'fill="{ink}"'), w


def roadmap(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["roadmap"]
    node_x, content_x = 64, 92
    content_w = W - content_x - 120
    out, nodes = [], []
    y = 0
    for i, (stage, text, items, state) in enumerate(s["stages"]):
        top = y + (2 if i == 0 else 16)
        out.append(f'<text x="40" y="{top + 16}" font-size="15" font-weight="700" text-anchor="end" fill="{t["text"]}">{esc(stage)}</text>')
        nodes.append((top + 11, state))
        parts, h = para(text, content_x, top, 14.5, content_w, t["text"], theme, lh=22)
        out += parts
        if items:
            ix, iy = content_x, top + h + 24
            for words, kind in items:
                suffix = {"next": s["next"], "todo": s["todo"]}.get(kind)
                svg_item, w = item(0, 0, words, kind, theme, suffix)
                if ix + w > content_x + content_w:
                    ix, iy = content_x, iy + 26
                out.append(group([svg_item], ix, iy))
                ix += w + 22
            h = iy + 6 - top
        ink = TONES[theme]["green" if state == "done" else "amber"]
        out.append(label(W, top + 16, s["status"][state], ink, 13.5, anchor="end"))
        y = top + max(h, 22) + 16
        if i < len(s["stages"]) - 1:
            out.append(rule(y, theme, x1=content_x))
    nodes_svg = [f'<line x1="{node_x}" y1="{nodes[0][0]}" x2="{node_x}" y2="{nodes[-1][0]}" stroke="{t["border"]}" stroke-width="2"/>']
    for cy, state in nodes:
        if state == "done":
            nodes_svg.append(f'<circle cx="{node_x}" cy="{cy}" r="6" fill="{TONES[theme]["green"]}"/>')
        else:
            nodes_svg.append(f'<circle cx="{node_x}" cy="{cy}" r="5.5" fill="{t["bg"]}" stroke="{TONES[theme]["amber"]}" stroke-width="2.5"/>')
    return page(y, theme, "\n".join(nodes_svg + out))


# 10. Repository.

def repository(theme, lang):
    t, s = THEMES[theme], TEXT[lang]["repository"]
    path_x, path_w, desc_x, desc_w = 132, 364, 520, 440
    out, y = [], 0
    for gi, (name, rows) in enumerate(s):
        if gi:
            out.append(rule(y, theme, strong=True))
        y += 14
        parts, _ = para(name, 0, y, 13, path_x - 16, t["accent"], theme, lh=20, weight=700)
        out += parts
        for path, desc in rows:
            p1, h1 = para(path, path_x, y, 13.5, path_w, t["text"], theme, lh=20)
            p2, h2 = para(desc, desc_x, y, 13.5, desc_w, t["muted"], theme, lh=20)
            out += p1 + p2
            y += max(h1, h2) + 6
        y += 8
    return page(y, theme, "\n".join(out))


CHARTS = dict(why=why, ai=ai_checks, measured=measured, more=more, platforms=platforms, technology=technology,
              capabilities=capabilities, choose=choose, roadmap=roadmap, repository=repository)

if __name__ == "__main__":
    for lang, suffix in LANGS:
        for theme in THEMES:
            for name, draw in CHARTS.items():
                (HERE / f"{name}{suffix}-{theme}.svg").write_text(draw(theme, lang))
