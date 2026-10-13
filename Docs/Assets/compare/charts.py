#!/usr/bin/env python3
"""Draw the comparison charts of the README as SVG, light and dark.

    python3 Docs/Assets/compare/charts.py

writes `routes-{light,dark}.svg` and `landscape-{light,dark}.svg` next to
this script. Framework logos are the Simple Icons paths in `icons.json`
(CC0; the marks belong to their owners); Bern's is its own logo. Text uses
the system fonts, so the files need nothing outside themselves.
"""

import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
ICONS = json.loads((HERE / "icons.json").read_text())
LOGO = (HERE.parent / "logo" / "bern-logo.svg").read_text()

FONT = ("-apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', "
        "'Microsoft YaHei', 'Noto Sans CJK SC', 'Helvetica Neue', Arial, sans-serif")

THEMES = {
    "light": dict(bg="#FFFFFF", panel="#F6F8FA", border="#D0D7DE", text="#1F2328", muted="#59636E",
                  grid="#E6EAEF", line="#AFB8C1", accent="#4A8BEF", soft="#EAF2FE", ring="#B9D3FA"),
    "dark": dict(bg="#0D1117", panel="#151B23", border="#30363D", text="#F0F6FC", muted="#9198A1",
                 grid="#21262D", line="#484F58", accent="#6EA2F5", soft="#14243D", ring="#2C4A78"),
}

FRAMEWORKS = {
    "bern": dict(name="Bern", color="#4A8BEF"),
    "gpui": dict(name="GPUI", color="#084CCF", icon="zedindustries"),
    "dioxus": dict(name="Dioxus", color="#64748B", letter="D"),
    "flutter": dict(name="Flutter", color="#02569B", icon="flutter"),
    "rn": dict(name="React Native", color="#20232A", icon="react", ink="#61DAFB"),
    "uniapp": dict(name="uni-app", color="#64748B", letter="U"),
    "uniappx": dict(name="uni-app x", color="#64748B", letter="X"),
    "electron": dict(name="Electron", color="#47848F", icon="electron"),
    "tauri": dict(name="Tauri", color="#24C8D8", icon="tauri"),
    "avalonia": dict(name="Avalonia", color="#8B44AC", icon="avaloniaui"),
    "qt": dict(name="Qt", color="#41CD52", icon="qt"),
}

TEXT = {
    "zh-Hans": dict(
        landscape_title="界面来源 × 随应用携带的运行时",
        landscape_note="按架构示意，不代表优劣",
        x_low="自绘或网页", x_high="系统原生控件", y_low="携带少", y_high="携带多",
        x_axis="界面元素来自", y_axis="随应用携带的运行时",
        routes_title="四条技术路线",
        routes_note="一份界面代码，怎样变成各平台上的界面",
        source="一份界面代码",
        lanes=[
            ("构建时生成", "原生界面代码", "界面就是系统控件", "外观与无障碍由系统负责"),
            ("运行时协调", "原生视图", "界面是系统控件", "由运行时驱动与桥接"),
            ("自带渲染引擎", "自己绘制界面", "各平台画出一样的界面", "随应用携带渲染引擎"),
            ("网页技术", "WebView 或浏览器", "界面是网页", "沿用网页生态"),
        ],
        notes={"bern": "SwiftUI · WinUI 3\nGTK 4 · Compose · Web", "electron": "内嵌 Chromium",
               "tauri": "系统 WebView", "dioxus": "系统 WebView", "uniapp": "经典模式",
               "uniappx": "编译为原生语言", "rn": "JS 引擎驱动", "qt": "Widgets 与 Quick"},
        native_band="系统控件：系统外观、无障碍与输入法",
        drawn_band="一致的界面：自己绘制，或携带浏览器",
        resources_title="Hello World 的资源占用",
        resources_note="Bern 为实测；其余为纸面估算，以后由 M4 上的 CI 实测替换",
        resources_note_measured="同一台机器实测：{machine}，macOS {macos}，release 构建，{runs} 次热启动的中位数",
        resources_note_partial="；斜线为未能构建的估算",
        resources_footer_measured=("包体为各框架自己的工具打出的结果：.app、自包含发布目录或 Rust 可执行文件；启动为从运行到窗口出现。",
                                   "内存为窗口出现 4 秒后的物理占用，含子进程与随之启动的 WebKit 进程。由 benchmarks/ 与 Benchmarks 工作流生成。"),
        panels=("包体大小（对数刻度）", "启动到窗口可见", "空闲内存"),
        measured="实测", estimated="估算", no_desktop="没有桌面应用目标",
        resources_footer=("Bern：Apple M4，release 构建，热启动 6 次的中位数。",
                          "估算按各框架 hello world 的典型构成：Electron、Tauri、Dioxus 的内存包含浏览器子进程，Qt 按 Widgets，Avalonia 按自包含发布。"),
        languages_title="前后端语言与执行方式",
        languages_note="界面层与逻辑层各用什么语言、怎样执行、两层之间如何通信",
        columns=("界面层", "逻辑层", "两层之间", "垃圾回收"),
        tiers=("原生编译，无 GC", "AOT 编译，有 GC", "JIT 运行，有 GC", "字节码解释，有 GC"),
        yes="有", no="无", partial="部分",
        languages_footer="Bern 一行指桌面目标；Web 目标的界面层是 JavaScript，逻辑层是编译为 WebAssembly 的 Rust。",
    ),
    "zh-Hant": dict(
        landscape_title="介面來源 × 隨應用程式攜帶的執行環境",
        landscape_note="依架構示意，不代表優劣",
        x_low="自繪或網頁", x_high="系統原生控制項", y_low="攜帶少", y_high="攜帶多",
        x_axis="介面元素來自", y_axis="隨應用程式攜帶的執行環境",
        routes_title="四條技術路線",
        routes_note="一份介面程式碼，怎樣變成各平台上的介面",
        source="一份介面程式碼",
        lanes=[
            ("建置時產生", "原生介面程式碼", "介面就是系統控制項", "外觀與無障礙由系統負責"),
            ("執行時協調", "原生視圖", "介面是系統控制項", "由執行環境驅動與橋接"),
            ("自帶繪製引擎", "自己繪製介面", "各平台畫出一樣的介面", "隨應用程式攜帶繪製引擎"),
            ("網頁技術", "WebView 或瀏覽器", "介面是網頁", "沿用網頁生態"),
        ],
        notes={"bern": "SwiftUI · WinUI 3\nGTK 4 · Compose · Web", "electron": "內嵌 Chromium",
               "tauri": "系統 WebView", "dioxus": "系統 WebView", "uniapp": "經典模式",
               "uniappx": "編譯為原生語言", "rn": "JS 引擎驅動", "qt": "Widgets 與 Quick"},
        native_band="系統控制項：系統外觀、無障礙與輸入法",
        drawn_band="一致的介面：自己繪製，或攜帶瀏覽器",
        resources_title="Hello World 的資源占用",
        resources_note="Bern 為實測；其餘為紙面估算，之後由 M4 上的 CI 實測取代",
        resources_note_measured="同一台機器實測：{machine}，macOS {macos}，release 建置，{runs} 次熱啟動的中位數",
        resources_note_partial="；斜線為未能建置的估算",
        resources_footer_measured=("套件大小為各框架自己的工具產生的結果：.app、自包含發佈目錄或 Rust 執行檔；啟動為從執行到視窗出現。",
                                   "記憶體為視窗出現 4 秒後的實體占用，含子行程與隨之啟動的 WebKit 行程。由 benchmarks/ 與 Benchmarks 工作流程產生。"),
        panels=("套件大小（對數刻度）", "啟動到視窗可見", "閒置記憶體"),
        measured="實測", estimated="估算", no_desktop="沒有桌面應用程式目標",
        resources_footer=("Bern：Apple M4，release 建置，熱啟動 6 次的中位數。",
                          "估算依各框架 hello world 的典型組成：Electron、Tauri、Dioxus 的記憶體包含瀏覽器子行程，Qt 以 Widgets 計，Avalonia 以自包含發佈計。"),
        languages_title="前後端語言與執行方式",
        languages_note="介面層與邏輯層各用什麼語言、怎樣執行、兩層之間如何溝通",
        columns=("介面層", "邏輯層", "兩層之間", "垃圾回收"),
        tiers=("原生編譯，無 GC", "AOT 編譯，有 GC", "JIT 執行，有 GC", "位元組碼直譯，有 GC"),
        yes="有", no="無", partial="部分",
        languages_footer="Bern 一列指桌面目標；Web 目標的介面層是 JavaScript，邏輯層是編譯為 WebAssembly 的 Rust。",
    ),
    "en": dict(
        landscape_title="Where the UI comes from × runtime shipped with the app",
        landscape_note="A sketch by architecture, not a ranking",
        x_low="Drawn or web", x_high="System controls", y_low="Ships less", y_high="Ships more",
        x_axis="UI elements come from", y_axis="Runtime shipped with the app",
        routes_title="Four ways to cross platforms",
        routes_note="How one UI codebase becomes the interface on each platform",
        source="One UI codebase",
        lanes=[
            ("Build-time", "native UI code", "UI is system controls", "Look follows the OS"),
            ("Runtime-driven", "native views", "UI is system controls", "Driven at runtime"),
            ("Own rendering", "engine", "Same UI everywhere", "Ships a renderer"),
            ("Web", "technology", "UI is a web page", "Reuses the web stack"),
        ],
        notes={"bern": "SwiftUI · WinUI 3\nGTK 4 · Compose · Web", "electron": "Bundles Chromium",
               "tauri": "System WebView", "dioxus": "System WebView", "uniapp": "Classic mode",
               "uniappx": "Compiles to native", "rn": "Driven by JS", "qt": "Widgets and Quick"},
        native_band="System controls: native look, accessibility, input methods",
        drawn_band="Same UI everywhere: own renderer or a browser",
        resources_title="Hello World footprint",
        resources_note="Bern is measured; the rest are paper estimates, to be replaced by CI runs on an M4",
        resources_note_measured="Measured on {machine}, macOS {macos}: release builds, median of {runs} warm starts",
        resources_note_partial="; hatched bars are estimates",
        resources_footer_measured=("Package: what each framework's own tools produce (.app, self-contained folder or Rust binary). Launch: start to window on screen.",
                                   "Memory: physical footprint 4 s after the window appears, child and WebKit processes included. From benchmarks/ and its workflow."),
        panels=("Package size (log scale)", "Launch to window", "Idle memory"),
        measured="Measured", estimated="Estimate", no_desktop="No desktop app target",
        resources_footer=("Bern: Apple M4, release build, median of 6 warm starts.",
                          "Estimates follow a typical hello world: Electron, Tauri and Dioxus memory includes browser processes; Qt uses Widgets; Avalonia is self-contained."),
        languages_title="Languages and execution",
        languages_note="What each layer is written in, how it runs and how the layers talk",
        columns=("UI layer", "Logic layer", "Between layers", "GC"),
        tiers=("Native, no GC", "AOT with GC", "JIT with GC", "Interpreted bytecode, GC"),
        yes="Yes", no="No", partial="Partly",
        languages_footer="The Bern row is for desktop targets; on the web the UI layer is JavaScript and the logic layer is Rust compiled to WebAssembly.",
    ),
    "ja": dict(
        landscape_title="UI の出どころ × アプリが同梱するランタイム",
        landscape_note="アーキテクチャの模式図で、優劣ではありません",
        x_low="自前描画・Web", x_high="システムのコントロール", y_low="少ない", y_high="多い",
        x_axis="UI 要素の出どころ", y_axis="同梱するランタイム",
        routes_title="四つの技術ルート",
        routes_note="一つの UI コードが、各プラットフォームの UI になるまで",
        source="一つの UI コード",
        lanes=[
            ("ビルド時に生成", "ネイティブ UI", "システムのコントロール", "外観は OS が担う"),
            ("実行時に", "ネイティブを駆動", "システムのコントロール", "ランタイムが橋渡し"),
            ("独自の", "描画エンジン", "どの OS でも同じ見た目", "描画エンジンを同梱"),
            ("Web 技術", "WebView など", "UI は Web ページ", "Web の資産を活用"),
        ],
        notes={"bern": "SwiftUI · WinUI 3\nGTK 4 · Compose · Web", "electron": "Chromium を同梱",
               "tauri": "システム WebView", "dioxus": "システム WebView", "uniapp": "クラシック",
               "uniappx": "ネイティブへコンパイル", "rn": "JS で駆動", "qt": "Widgets と Quick"},
        native_band="システムのコントロール：OS の外観・アクセシビリティ・入力",
        drawn_band="同じ見た目：自前で描画するか、ブラウザを同梱",
        resources_title="Hello World のリソース",
        resources_note="Bern は実測、ほかは机上の見積もり。M4 の CI 実測で置き換える予定",
        resources_note_measured="同一マシンで実測：{machine}、macOS {macos}、release ビルド、ウォームスタート {runs} 回の中央値",
        resources_note_partial="。斜線は見積もり",
        resources_footer_measured=("パッケージは各フレームワーク自身のツールが作るもの（.app、自己完結フォルダー、Rust 実行ファイル）。起動は開始から表示まで。",
                                   "メモリは表示 4 秒後の物理フットプリントで、子プロセスと同時に起動した WebKit プロセスを含む。benchmarks/ のワークフローで生成。"),
        panels=("パッケージサイズ（対数）", "起動からウィンドウ表示", "アイドル時のメモリ"),
        measured="実測", estimated="見積もり", no_desktop="デスクトップ向けなし",
        resources_footer=("Bern：Apple M4、release ビルド、ウォームスタート 6 回の中央値。",
                          "見積もりは典型的な hello world：Electron・Tauri・Dioxus のメモリはブラウザのプロセスを含み、Qt は Widgets、Avalonia は自己完結型。"),
        languages_title="言語と実行方式",
        languages_note="UI 層とロジック層の言語、実行のしかた、層の間の通信",
        columns=("UI 層", "ロジック層", "層の間", "GC"),
        tiers=("ネイティブ、GC なし", "AOT、GC あり", "JIT、GC あり", "バイトコード解釈、GC あり"),
        yes="あり", no="なし", partial="一部",
        languages_footer="Bern の行はデスクトップ向け。Web では UI 層が JavaScript、ロジック層は WebAssembly にコンパイルした Rust。",
    ),
}


def esc(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def width(text, size):
    """Approximate rendered width: CJK characters are square, Latin narrower."""
    total = 0.0
    for c in text:
        if ord(c) > 0x2E80 or c in "→×":
            total += 1.0
        else:
            total += 0.32 if c in " ·.,'" else 0.6
    return total * size


def bern_symbol():
    """The Bern logo as a reusable symbol."""
    body = LOGO[LOGO.index("<defs>"):LOGO.rindex("</svg>")]
    body = re.sub(r"<!--.*?-->", "", body, flags=re.S)
    body = body.replace('id="bern-disc"', 'id="bern-symbol-disc"').replace("url(#bern-disc)", "url(#bern-symbol-disc)")
    return f'<symbol id="bern-logo" viewBox="0 0 1024 1024">{body}</symbol>'


def badge(key, cx, cy, r):
    """A framework's mark in a disc of radius `r`."""
    fw = FRAMEWORKS[key]
    if key == "bern":
        return f'<use href="#bern-logo" x="{cx - r:.1f}" y="{cy - r:.1f}" width="{2 * r:.1f}" height="{2 * r:.1f}"/>'
    out = [f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fw["color"]}"/>']
    if "icon" in fw:
        size = r * 1.1
        scale = size / 24
        ink = fw.get("ink", "#FFFFFF")
        for d in ICONS[fw["icon"]]["paths"]:
            out.append(f'<path d="{d}" fill="{ink}" transform="translate({cx - size / 2:.2f} {cy - size / 2:.2f}) scale({scale:.4f})"/>')
    else:
        out.append(f'<text x="{cx:.1f}" y="{cy + r * 0.36:.1f}" text-anchor="middle" font-size="{r:.1f}" '
                   f'font-weight="700" fill="#FFFFFF">{fw["letter"]}</text>')
    return "".join(out)


def svg(width_, height, theme, body, bottom=None):
    """The chart with no frame or background, cropped to the content between x 36 and 944."""
    t = THEMES[theme]
    left, top = 36, 28
    w, h = width_ - 2 * left, (bottom or height) - top
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{left} {top} {w} {h}" width="{w}" height="{h}" '
            f'font-family="{FONT}">\n<defs>{bern_symbol()}'
            f'<marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" orient="auto-start-reverse">'
            f'<path d="M0 0 L10 5 L0 10 z" fill="{t["line"]}"/></marker>'
            f'<marker id="arrow-accent" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="11" markerHeight="11" markerUnits="userSpaceOnUse" orient="auto-start-reverse">'
            f'<path d="M0 0 L10 5 L0 10 z" fill="{t["accent"]}"/></marker></defs>\n'
            f'{body}\n</svg>\n')


def landscape(theme, lang):
    t, s = THEMES[theme], TEXT[lang]
    W, H = 960, 640
    x0, y0, w, h = 120, 104, 780, 432
    out = [
        f'<text x="40" y="52" font-size="22" font-weight="700" fill="{t["text"]}">{esc(s["landscape_title"])}</text>',
        f'<text x="40" y="78" font-size="14" fill="{t["muted"]}">{esc(s["landscape_note"])}</text>',
        # Plot: the native, light quadrant is tinted.
        f'<rect x="{x0}" y="{y0}" width="{w}" height="{h}" rx="14" fill="{t["panel"]}"/>',
        f'<path d="M{x0 + w / 2} {y0 + h / 2} H{x0 + w} V{y0 + h - 14} A14 14 0 0 1 {x0 + w - 14} {y0 + h} H{x0 + w / 2} Z" fill="{t["soft"]}"/>',
        f'<line x1="{x0 + w / 2}" y1="{y0 + 10}" x2="{x0 + w / 2}" y2="{y0 + h - 10}" stroke="{t["grid"]}" stroke-width="1.5" stroke-dasharray="4 6"/>',
        f'<line x1="{x0 + 10}" y1="{y0 + h / 2}" x2="{x0 + w - 10}" y2="{y0 + h / 2}" stroke="{t["grid"]}" stroke-width="1.5" stroke-dasharray="4 6"/>',
        # Axes.
        f'<line x1="{x0}" y1="{y0 + h + 22}" x2="{x0 + w}" y2="{y0 + h + 22}" stroke="{t["line"]}" stroke-width="1.5" marker-end="url(#arrow)"/>',
        f'<text x="{x0}" y="{y0 + h + 48}" font-size="14" fill="{t["muted"]}">{esc(s["x_low"])}</text>',
        f'<text x="{x0 + w}" y="{y0 + h + 48}" font-size="14" font-weight="600" text-anchor="end" fill="{t["text"]}">{esc(s["x_high"])}</text>',
        f'<text x="{x0 + w / 2}" y="{y0 + h + 48}" font-size="13" text-anchor="middle" fill="{t["muted"]}">{esc(s["x_axis"])}</text>',
        f'<line x1="{x0 - 22}" y1="{y0 + h}" x2="{x0 - 22}" y2="{y0}" stroke="{t["line"]}" stroke-width="1.5" marker-end="url(#arrow)"/>',
        f'<text x="{x0 - 34}" y="{y0 + h}" font-size="14" text-anchor="end" fill="{t["muted"]}">{esc(s["y_low"])}</text>',
        f'<text x="{x0 - 34}" y="{y0 + 14}" font-size="14" text-anchor="end" fill="{t["muted"]}">{esc(s["y_high"])}</text>',
        f'<text transform="translate({x0 - 46} {y0 + h / 2}) rotate(-90)" font-size="13" text-anchor="middle" fill="{t["muted"]}">{esc(s["y_axis"])}</text>',
    ]
    points = [
        ("electron", 0.06, 0.93), ("avalonia", 0.28, 0.67), ("qt", 0.47, 0.60), ("flutter", 0.15, 0.53),
        ("rn", 0.80, 0.45), ("uniapp", 0.26, 0.40), ("uniappx", 0.66, 0.30), ("dioxus", 0.22, 0.25),
        ("tauri", 0.06, 0.19), ("gpui", 0.14, 0.07), ("bern", 0.93, 0.08),
    ]
    for key, px, py in points:
        cx, cy = x0 + px * w, y0 + (1 - py) * h
        name = FRAMEWORKS[key]["name"]
        if key == "bern":
            r = 30
            out.append(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r + 9}" fill="{t["ring"]}" opacity="0.55"/>')
            out.append(badge(key, cx, cy, r))
            out.append(f'<text x="{cx - r - 14:.1f}" y="{cy + 7:.1f}" font-size="19" font-weight="700" text-anchor="end" fill="{t["text"]}">{name}</text>')
            continue
        r = 18
        out.append(badge(key, cx, cy, r))
        out.append(f'<text x="{cx + r + 9:.1f}" y="{cy + 5:.1f}" font-size="15" font-weight="600" fill="{t["text"]}">{esc(name)}</text>')
    return svg(W, H, theme, "\n".join(out), bottom=y0 + h + 56)


def routes(theme, lang):
    t, s = THEMES[theme], TEXT[lang]
    W, H = 960, 600
    out = [
        f'<text x="40" y="52" font-size="22" font-weight="700" fill="{t["text"]}">{esc(s["routes_title"])}</text>',
        f'<text x="40" y="78" font-size="14" fill="{t["muted"]}">{esc(s["routes_note"])}</text>',
    ]
    # The source.
    out.append(f'<text x="480" y="126" font-size="17" font-weight="700" text-anchor="middle" fill="{t["text"]}">{esc(s["source"])}</text>')
    lanes = [["bern"], ["rn", "uniappx"], ["flutter", "gpui", "avalonia", "qt"], ["electron", "tauri", "dioxus", "uniapp"]]
    card_w, gap, left, top, card_h = 204, 28, 40, 200, 300
    for index, (lane, (title1, title2, line1, line2)) in enumerate(zip(lanes, s["lanes"])):
        x = left + index * (card_w + gap)
        cx = x + card_w / 2
        bern = index == 0
        stroke = t["accent"] if bern else t["line"]
        out.append(f'<path d="M480 138 C 480 170, {cx:.1f} 160, {cx:.1f} {top - 6}" fill="none" stroke="{stroke}" '
                   f'stroke-width="{2.5 if bern else 1.5}" marker-end="url(#{"arrow-accent" if bern else "arrow"})"/>')
        if index:
            out.append(f'<line x1="{x - gap / 2}" y1="{top + 4}" x2="{x - gap / 2}" y2="{top + card_h - 4}" stroke="{t["grid"]}"/>')
        x -= 12
        out.append(f'<text x="{x + 20}" y="{top + 30}" font-size="24" font-weight="700" fill="{t["accent"] if bern else t["line"]}">{index + 1}</text>')
        out.append(f'<text x="{x + 46}" y="{top + 28}" font-size="16" font-weight="700" fill="{t["accent"] if bern else t["text"]}">{esc(title1)}</text>')
        out.append(f'<text x="{x + 46}" y="{top + 49}" font-size="16" font-weight="700" fill="{t["accent"] if bern else t["text"]}">{esc(title2)}</text>')
        out.append(f'<text x="{x + 20}" y="{top + 82}" font-size="13.5" fill="{t["muted"]}">{esc(line1)}</text>')
        out.append(f'<text x="{x + 20}" y="{top + 102}" font-size="13.5" fill="{t["muted"]}">{esc(line2)}</text>')
        for row, key in enumerate(lane):
            ry = top + 150 + row * 38
            r = 22 if bern else 14
            if bern:
                ry += 10
            out.append(badge(key, x + 20 + r, ry, r))
            name = FRAMEWORKS[key]["name"]
            note = s["notes"].get(key)
            tx = x + 20 + 2 * r + 10
            if bern:
                out.append(f'<text x="{tx}" y="{ry + 7}" font-size="20" font-weight="700" fill="{t["text"]}">{name}</text>')
                for line, part in enumerate(note.split("\n") if note else []):
                    out.append(f'<text x="{x + 20}" y="{ry + 52 + line * 20}" font-size="13" fill="{t["muted"]}">{esc(part)}</text>')
            else:
                out.append(f'<text x="{tx}" y="{ry + 1 if note else ry + 5}" font-size="14.5" font-weight="600" fill="{t["text"]}">{esc(name)}</text>')
                if note:
                    out.append(f'<text x="{tx}" y="{ry + 17}" font-size="12.5" fill="{t["muted"]}">{esc(note)}</text>')
    # What each half of the routes gives.
    band_y = top + card_h + 26
    for start, label, tone in ((0, s["native_band"], "accent"), (2, s["drawn_band"], "line")):
        bx = left + start * (card_w + gap)
        bw = 2 * card_w + gap
        out.append(f'<path d="M{bx + 6} {band_y - 10} v10 h{bw - 12} v-10" fill="none" stroke="{t[tone]}" stroke-width="1.5"/>')
        out.append(f'<text x="{bx + bw / 2}" y="{band_y + 24}" font-size="13.5" font-weight="600" text-anchor="middle" fill="{t["text"] if tone == "accent" else t["muted"]}">{esc(label)}</text>')
    return svg(W, H, theme, "\n".join(out), bottom=band_y + 34)


# Hello world on Apple Silicon, release builds: package (MB), start (s) and
# idle memory (MB). Bern is measured; the rest are paper estimates until a CI
# benchmark replaces them.
RESOURCES = [
    ("bern", 2.0, 0.28, 20, True),
    ("gpui", 12, 0.25, 45, False),
    ("qt", 40, 0.35, 40, False),
    ("tauri", 4, 0.45, 85, False),
    ("dioxus", 6, 0.45, 85, False),
    ("flutter", 45, 0.55, 70, False),
    ("avalonia", 45, 0.65, 75, False),
    ("rn", 25, 0.90, 85, False),
    ("electron", 230, 1.00, 160, False),
    ("uniapp", None, None, None, False),
]

# Measurements from the Benchmarks workflow, when its `results.json` is in
# benchmarks/: the same Hello World on one machine. Frameworks it could not
# build stay estimates. Rows go from the smallest package to the largest.
RESULTS = HERE.parents[2] / "benchmarks" / "results.json"
MEASURED = {}
if RESULTS.exists():
    for result in json.loads(RESULTS.read_text())["results"]:
        MEASURED["rn" if result["framework"] == "react-native" else result["framework"]] = result
    RESOURCES = sorted(
        (
            (key, MEASURED[key]["package_bytes"] / 1e6, MEASURED[key]["launch_s"], MEASURED[key]["memory_bytes"] / 1e6, True)
            if key in MEASURED
            else (key, size, start, memory, False)
            for key, size, start, memory, _ in RESOURCES
        ),
        key=lambda row: (row[1] is None, row[1] or 0),
    )

def hatch(theme):
    t = THEMES[theme]
    return (f'<pattern id="hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">'
            f'<rect width="6" height="6" fill="{t["grid"]}"/><rect width="3" height="6" fill="{t["line"]}" opacity="0.55"/></pattern>')


def resources(theme, lang):
    import math
    t, s = THEMES[theme], TEXT[lang]
    if MEASURED:
        sample = next(iter(MEASURED.values()))
        note = s["resources_note_measured"].format(
            machine=sample["machine"], macos=sample["macos"], runs=len(sample["launch_runs_s"]))
        if len(MEASURED) < sum(1 for row in RESOURCES if row[1] is not None):
            note += s["resources_note_partial"]
        footer = list(s["resources_footer_measured"])
    else:
        note, footer = s["resources_note"], s["resources_footer"]
    W = 960
    top, row_h = 150, 40
    H = top + row_h * len(RESOURCES) + 92 + 20 * (len(footer) - 2)
    out = [
        f'<defs>{hatch(theme)}</defs>',
        f'<text x="40" y="52" font-size="22" font-weight="700" fill="{t["text"]}">{esc(s["resources_title"])}</text>',
        f'<text x="40" y="78" font-size="14" fill="{t["muted"]}">{esc(note)}</text>',
    ]
    # Legend, laid out from the right edge.
    right = W - 40
    for label, fill in ((s["estimated"], "url(#hatch)"), (s["measured"], t["accent"])):
        text_x = right - width(label, 13)
        out.append(f'<text x="{text_x:.1f}" y="51" font-size="13" fill="{t["text"]}">{esc(label)}</text>')
        out.append(f'<rect x="{text_x - 30:.1f}" y="40" width="22" height="12" rx="4" fill="{fill}"/>')
        right = text_x - 48
    panels = [(250, 210), (485, 210), (720, 210)]
    for (px, pw), title in zip(panels, s["panels"]):
        out.append(f'<text x="{px}" y="{top - 22}" font-size="13.5" font-weight="600" fill="{t["muted"]}">{esc(title)}</text>')
        out.append(f'<line x1="{px}" y1="{top - 10}" x2="{px}" y2="{top + row_h * len(RESOURCES) - 6}" stroke="{t["border"]}"/>')
    # Room for the largest value of each panel.
    largest = [max((row[i] for row in RESOURCES if row[i] is not None), default=1) for i in (1, 2, 3)]
    scales = [
        lambda v: math.log10(max(v, 1)) / math.log10(max(300, largest[0] * 1.1)),
        lambda v: v / max(1.2, largest[1] * 1.1),
        lambda v: v / max(180, largest[2] * 1.1),
    ]
    units = [lambda v: f"{v:.1f} MB" if v < 10 else f"{v:.0f} MB", lambda v: f"{v:.2f} s", lambda v: f"{v:.0f} MB"]
    for index, (key, size, start, memory, measured) in enumerate(RESOURCES):
        cy = top + index * row_h + row_h / 2 - 6
        out.append(badge(key, 56, cy, 13))
        out.append(f'<text x="78" y="{cy + 5}" font-size="14.5" font-weight="{700 if measured else 600}" fill="{t["text"]}">{esc(FRAMEWORKS[key]["name"])}</text>')
        if size is None:
            out.append(f'<text x="{panels[0][0] + 10}" y="{cy + 5}" font-size="13" fill="{t["muted"]}">{esc(s["no_desktop"])}</text>')
            continue
        for panel, ((px, pw), value, scale, unit) in enumerate(zip(panels, (size, start, memory), scales, units)):
            bar = max(4, min(1.0, scale(value)) * (pw - 70))
            fill = t["accent"] if measured else "url(#hatch)"
            out.append(f'<rect x="{px + 1}" y="{cy - 7}" width="{bar:.1f}" height="14" rx="5" fill="{fill}"/>')
            label = unit(value) if measured else "≈" + unit(value)
            out.append(f'<text x="{px + bar + 8:.1f}" y="{cy + 5}" font-size="12.5" font-weight="{700 if measured else 500}" '
                       f'fill="{t["text"] if measured else t["muted"]}">{esc(label)}</text>')
    for line, text in enumerate(footer):
        out.append(f'<text x="40" y="{H - 28 - 20 * (len(footer) - 1) + line * 20}" font-size="12.5" fill="{t["muted"]}">{esc(text)}</text>')
    return svg(W, H, theme, "\n".join(out), bottom=H - 16)


# Interface layer, logic layer, how they talk, garbage collection. Tiers:
# 0 native, 1 AOT with GC, 2 JIT with GC, 3 interpreted bytecode.
LANGUAGES = {
    "zh-Hans": [
        ("bern", [("Swift · C++ · Rust", 0)], [("Rust", 0)], "同进程 C ABI", "no"),
        ("gpui", [("Rust", 0)], [("Rust", 0)], "同一语言", "no"),
        ("qt", [("C++", 0), ("QML", 2)], [("C++", 0)], "同一进程", "partial"),
        ("tauri", [("JavaScript", 2)], [("Rust", 0)], "WebView 消息", "partial"),
        ("dioxus", [("Rust → WebView", 0)], [("Rust", 0)], "向 WebView 发补丁", "partial"),
        ("flutter", [("Dart", 1)], [("Dart", 1)], "同一语言", "yes"),
        ("avalonia", [("C#", 2)], [("C#", 2)], "同一语言", "yes"),
        ("rn", [("JavaScript", 3)], [("JavaScript", 3)], "JSI 调用原生视图", "yes"),
        ("uniapp", [("JavaScript", 2), ("UTS → 原生", 0)], [("JavaScript", 2)], "WebView 或原生", "yes"),
        ("electron", [("JavaScript", 2)], [("JavaScript", 2)], "进程间 IPC", "yes"),
    ],
}


def _localized(bridges, uts):
    rows = []
    for (key, ui, logic, _, gc), bridge in zip(LANGUAGES["zh-Hans"], bridges):
        ui = [(uts if label == "UTS → 原生" else label, tier) for label, tier in ui]
        rows.append((key, ui, logic, bridge, gc))
    return rows


LANGUAGES["zh-Hant"] = _localized(
    ["同行程 C ABI", "同一語言", "同一行程", "WebView 訊息", "向 WebView 送出差異", "同一語言", "同一語言",
     "JSI 呼叫原生視圖", "WebView 或原生", "行程間 IPC"], "UTS → 原生")
LANGUAGES["en"] = _localized(
    ["C ABI, same process", "Same language", "Same process", "WebView messages", "DOM patches to WebView",
     "Same language", "Same language", "JSI to native views", "WebView or native", "IPC between processes"],
    "UTS → native")
LANGUAGES["ja"] = _localized(
    ["同一プロセスの C ABI", "同じ言語", "同一プロセス", "WebView へメッセージ", "WebView へ差分を送る",
     "同じ言語", "同じ言語", "JSI でネイティブビュー", "WebView かネイティブ", "プロセス間 IPC"], "UTS→ネイティブ")
TIER_COLORS = ["#22A06B", "#4A8BEF", "#E2A03F", "#E5684D"]


def languages(theme, lang):
    t, s = THEMES[theme], TEXT[lang]
    rows = LANGUAGES[lang]
    W, top, row_h = 960, 150, 42
    H = top + row_h * len(rows) + 66
    out = [
        f'<text x="40" y="52" font-size="22" font-weight="700" fill="{t["text"]}">{esc(s["languages_title"])}</text>',
        f'<text x="40" y="78" font-size="14" fill="{t["muted"]}">{esc(s["languages_note"])}</text>',
    ]
    x = 40
    for color, label in zip(TIER_COLORS, s["tiers"]):
        out.append(f'<circle cx="{x + 6}" cy="{106}" r="6" fill="{color}"/>')
        out.append(f'<text x="{x + 18}" y="{111}" font-size="13" fill="{t["text"]}">{esc(label)}</text>')
        x += width(label, 13) + 48
    columns = [(230, s["columns"][0]), (500, s["columns"][1]), (668, s["columns"][2]), (866, s["columns"][3])]
    for cx, title in columns:
        out.append(f'<text x="{cx}" y="{top - 12}" font-size="13.5" font-weight="600" fill="{t["muted"]}">{esc(title)}</text>')

    def chips(x, y, items):
        parts = []
        for label, tier in items:
            parts.append(f'<circle cx="{x + 5}" cy="{y}" r="5" fill="{TIER_COLORS[tier]}"/>')
            parts.append(f'<text x="{x + 16}" y="{y + 4.5}" font-size="13.5" font-weight="600" fill="{t["text"]}">{esc(label)}</text>')
            x += width(label, 13.5) + 34
        return parts

    for index, (key, ui, logic, bridge, gc) in enumerate(rows):
        cy = top + index * row_h + row_h / 2
        bern = key == "bern"
        out.append(f'<line x1="40" y1="{cy - row_h / 2:.1f}" x2="920" y2="{cy - row_h / 2:.1f}" stroke="{t["border"] if index == 0 else t["grid"]}"/>')
        out.append(badge(key, 53, cy, 13))
        out.append(f'<text x="75" y="{cy + 5}" font-size="14.5" font-weight="{700 if bern else 600}" fill="{t["accent"] if bern else t["text"]}">{esc(FRAMEWORKS[key]["name"])}</text>')
        out.extend(chips(230, cy, ui))
        out.extend(chips(500, cy, logic))
        out.append(f'<text x="668" y="{cy + 5}" font-size="13.5" fill="{t["text"]}">{esc(bridge)}</text>')
        mark = {"no": (s["no"], TIER_COLORS[0]), "partial": (s["partial"], TIER_COLORS[2]), "yes": (s["yes"], t["muted"])}[gc]
        out.append(f'<text x="866" y="{cy + 5}" font-size="13.5" font-weight="600" fill="{mark[1]}">{esc(mark[0])}</text>')
    out.append(f'<text x="40" y="{H - 26}" font-size="12.5" fill="{t["muted"]}">{esc(s["languages_footer"])}</text>')
    return svg(W, H, theme, "\n".join(out), bottom=H - 16)


if __name__ == "__main__":
    for lang, suffix in (("zh-Hans", ""), ("zh-Hant", "-zh-Hant"), ("en", "-en"), ("ja", "-ja")):
        for theme in THEMES:
            (HERE / f"routes{suffix}-{theme}.svg").write_text(routes(theme, lang))
            (HERE / f"landscape{suffix}-{theme}.svg").write_text(landscape(theme, lang))
            (HERE / f"resources{suffix}-{theme}.svg").write_text(resources(theme, lang))
            (HERE / f"languages{suffix}-{theme}.svg").write_text(languages(theme, lang))
