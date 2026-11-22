[English](../../Guides/Localization-and-Styling.md) · **简体中文**

# 本地化、主题与 `style!` 动效

Bern 将多语言、深浅色主题、动效与加载骨架图全部纳入编译期声明，开箱即用而无需手写繁琐的平台样式代码。

## 1. 行内本地化（`tr!`）

在 `Bern.toml` 中声明应用支持的语言：

```toml
[languages]
supported = ["zh-Hans", "en"]
default = "en"
strict = true   # 默认开启：缺少翻译报 B102，直接写硬编码展示字面量报 B103
```

使用 `tr!` 把多语言文案直接写在控件旁边：

```rust
Text(content: tr! {
    "zh-Hans" => "待办（剩余 {n} 项）",
    "en" => "{n, plural, =0 {All done} one {1 task left} other {# tasks left}}";
    n: app.remaining
})
```

- **编译期检查**：少写任何一种语言报 `error[B102]`；严格模式下在展示属性中直接写字面量报 `error[B103]`；不同语言之间插值参数不一致也无法通过编译。
- **消息格式化**：支持命名插值、ICU 复数分支（`plural`）、日期格式（`{date, date, y-MM-dd}`）与时长格式（详见[当前组件用法 — 文本内容与格式化](../Reference/UI.md#文本内容与格式化)）。
- **运行时热切换**：在设置页放一个 `LanguagePicker` 或通过 CLI 调用 `ui set-language --language en`，全端即时更新文案且保留当前窗口草稿。

## 2. 零配置深浅色与品牌色

每个 Bern 后端都内置浅色、深色与高对比度适配，应用默认无需为控件配置颜色：

```toml
[theme]
mode = "system"        # "system"（默认）、"light" 或 "dark"
brand_seed = "#2F6FE4" # 可选品牌强调色
```

- 在 **SwiftUI、WinUI 3、GTK 4 与 Compose** 上，颜色直接取自系统主题，`brand_seed` 映射为系统强调色。
- 在 **Web 与 GPUI** 上，`bern-theme` 根据 `brand_seed` 推导整套通过对比度校验的浅色、深色与高对比度配色。
- 需要局部固定外观（例如浅色窗口内的深色视频画框）时，使用 `Stack(appearance: dark) { ... }`；需要运行时切换主题与强调色时，在 `WindowSet(appearance:, accent:)` 上绑定状态。
- 开发调试时可用环境变量 `BERN_APPEARANCE=dark` 或 `BERN_APPEARANCE=light` 临时覆盖系统外观。

## 3. 使用 `style!` 自定义样式与弹簧动效

当应用需要品牌化容器样式、弹簧物理动效或悬停/按下态过渡时，在 UI crate 中编写 `style!` 块：

```rust
style! {
    motion {
        snappy: spring(period: 0.24, damping: 0.85),
        jelly: spring(period: 0.42, damping: 0.45),
    }

    transition {
        pop_in: pop(from: 0.85, jelly),
    }

    Button.hero {
        background: accent,
        corner: 10,
        size: large,
        hover: scale(1.03, snappy),
        pressed: scale(0.96, snappy),
        settle: jelly,
    }
}
```

通过 `variant: hero` 引用具名样式变体，通过 `appear:`、`disappear:`、`stagger:`、`reflow:` 与共享元素 `hero:` / `hero_key:` 声明结构过渡。

- 所有后端均自动遵循系统的“减少动态效果”设置；本地调试时可使用 `BERN_MOTION=reduce` 模拟减少动态效果，或在 SwiftUI / GPUI 上用 `BERN_MOTION=slow` 以 10 倍慢放逐帧检查动效。

## 4. 编译期自动骨架图

Bern 在编译期直接从你的真实 `ui!` 树投影加载骨架图，不需要维护第二套占位界面：

- 在 `Grid`、`Column`、`Row` 等容器上绑定 `skeleton: <bool>`（可选 `skeleton_count: <n>`），或让 `Query` 在加载期自动展示投影骨架图。
- 内置 **120 ms 防闪烁延迟**（快速响应时不闪现骨架）、数据就绪后的轻浮错峰入场，并在全六端自动遵循无障碍播报与减少动态效果。
- 需要精细手排占位块时，可直接使用 `Skeleton(width:, height:, shape:, tone:)` 组件。
