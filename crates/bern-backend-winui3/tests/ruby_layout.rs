//! Execute the actual LayoutUpdated body with XAML-like property carriers.
//! Setters invalidate layout and may store double-valued API dimensions as
//! floats. Both exact and native-precision round trips must converge without
//! further property writes or drawing.

#[cfg(not(windows))]
#[test]
fn ruby_layout_reaches_a_fixed_point() {
    let widgets = include_str!("../templates/cpp/bern_ui_widgets.inl");
    let ruby = widgets
        .split_once("inline xaml::FrameworkElement WithRuby(")
        .unwrap()
        .1;
    let callback = ruby
        .split_once("block.LayoutUpdated(")
        .unwrap()
        .1
        .split_once("(auto&&, auto&&) {")
        .unwrap()
        .1
        .split_once("\n    });")
        .unwrap()
        .0;
    let stamp = ruby
        .split_once("    struct Stamp ")
        .unwrap()
        .1
        .split_once('\n')
        .unwrap()
        .0;
    let code = r#"
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <functional>
#include <memory>
#include <string>
#include <vector>
namespace winrt { using hstring = std::string; }
namespace xaml {
    enum class LineStackingStrategy { MaxHeight, BlockLineHeight };
    struct Thickness { double Left = 0, Top = 0, Right = 0, Bottom = 0; };
}
struct FontWeight { uint16_t Weight = 400; };
struct State {
    std::string text = "plain", annotations;
    double size = 26, height = 0, width = 400, actualHeight = 32;
    xaml::Thickness margin{1, 2, 3, 4};
    xaml::LineStackingStrategy strategy = xaml::LineStackingStrategy::MaxHeight;
    int writes = 0, draws = 0;
    bool mounted = true, nativeFloats = false;
    double stored(double value) const { return nativeFloats ? static_cast<float>(value) : value; }
};
struct Block {
    State* state;
    explicit operator bool() const { return state->mounted; }
    auto Text() const { return state->text; }
    double FontSize() const { return state->size; }
    auto FontWeight() const -> ::FontWeight { return {}; }
    double ActualWidth() const { return state->width; }
    double ActualHeight() const { return state->actualHeight; }
    auto LineStackingStrategy() const { return state->strategy; }
    void LineStackingStrategy(xaml::LineStackingStrategy value) { state->strategy = value; ++state->writes; }
    double LineHeight() const { return state->height; }
    void LineHeight(double value) { state->height = state->stored(value); ++state->writes; }
    auto Margin() const { return state->margin; }
    void Margin(xaml::Thickness value) { value.Top = state->stored(value.Top); state->margin = value; ++state->writes; }
};
"#;
    let code = format!("{code}\n{}", r#"
struct Weak { State* state; Block get() const { return {state}; } };
std::vector<int> RubySourceRanges(std::string, std::string, std::string ruby, bool) {
    return ruby.empty() || ruby == "[]" ? std::vector<int>{} : std::vector<int>{1};
}
void DrawRuby(Block block, int, std::vector<int> const&, int) { ++block.state->draws; }
void check(bool nativeFloats) {
    State state;
    state.nativeFloats = nativeFloats;
    auto weak = Weak{&state}, target = weak;
    auto ranges = std::make_shared<std::vector<int>>();
    std::function<std::string()> original = [&] { return state.text; };
    auto source = [&] { return state.annotations; };
    bool markdown = false;
    auto originalStrategy = state.strategy;
    auto originalMargin = state.margin;
    auto minimum = std::make_shared<double>(0);
    auto revision = std::make_shared<std::uint64_t>(0);
    int layer = 0, play = 0;
    struct Stamp STAMP
    auto stamp = std::make_shared<Stamp>();
    auto update = [&] { CALLBACK };
    auto settle = [&] {
        // A changed geometry may need another pass, but must converge.
        bool stable = false;
        for (int pass = 0; pass < 8; ++pass) {
            int writes = state.writes, draws = state.draws;
            update();
            if (writes == state.writes && draws == state.draws) { stable = true; break; }
        }
        assert(stable);
        int writes = state.writes, draws = state.draws;
        for (int pass = 0; pass < 100; ++pass) update();
        assert(state.writes == writes && state.draws == draws);
    };
    for (auto content : {"", "plain", "漢字", "line\nwrap"}) {
        state.text = content;
        for (auto annotations : {"", "[]", "[[0,2,\"reading\"]]", ""}) {
            state.annotations = annotations;
            *ranges = RubySourceRanges(state.text, state.text, state.annotations, false);
            ++*revision;
            for (double size : {21.0, 26.0, 26.125, 31.99999, 32.0, 26.0}) {
                state.size = size;
                settle();
                bool annotated = !ranges->empty();
                assert(state.strategy == (annotated ? xaml::LineStackingStrategy::BlockLineHeight : originalStrategy));
                assert(static_cast<float>(state.height) == static_cast<float>(annotated ? size * 1.65 : *minimum));
                assert(static_cast<float>(state.margin.Top) == static_cast<float>(originalMargin.Top + (annotated ? size * 0.55 : 0)));
                assert(state.margin.Left == 1 && state.margin.Right == 3 && state.margin.Bottom == 4);
            }
            state.width = 180;
            state.actualHeight = 96;
            settle();
        }
    }
    *minimum = 80;
    state.annotations = "reading";
    *ranges = {1};
    ++*revision;
    settle();
    assert(state.height == 80);
    state.mounted = false;
    state.text = "unmounted";
    settle();
}
int main() { check(false); check(true); }
"#.replace("STAMP", stamp).replace("CALLBACK", callback));
    let dir = std::env::temp_dir().join(format!("bern-ruby-layout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("layout.cpp");
    let binary = dir.join("layout");
    std::fs::write(&source, code).unwrap();
    let compiled = std::process::Command::new("c++")
        .args(["-std=c++20", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = std::process::Command::new(binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
