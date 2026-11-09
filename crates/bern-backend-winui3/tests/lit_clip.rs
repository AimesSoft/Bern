//! Execute the production line/clip geometry with native text coordinates.
//! A glyph's leading bearing may put its caret left of the text origin.

#[cfg(not(windows))]
#[test]
fn timed_highlight_preserves_negative_text_origins() {
    let widgets = include_str!("../templates/cpp/bern_ui_widgets.inl");
    let play = widgets.split_once("inline void PlayLit(").unwrap().1;
    let line = play
        .split_once("    auto sameLine =")
        .unwrap()
        .1
        .split_once("    if (play->lit")
        .unwrap()
        .0;
    let clip = play
        .split_once("    auto top =")
        .unwrap()
        .1
        .split_once("    over.Opacity")
        .unwrap()
        .0;
    let keyframe = play
        .lines()
        .find(|line| line.contains("motion.InsertKeyFrame(progress,"))
        .unwrap();
    let source = r#"
#include <algorithm>
#include <cassert>
#include <cmath>
#include <memory>
#include <vector>
struct float2 { float x, y; };
struct Rect { float X, Y, Width, Height; };
struct Clip {
    float2 offset{}, size{};
    void StopAnimation(const wchar_t*) {}
    void Offset(float2 at) { offset = at; }
    void Size(float2 value) { assert(value.x >= 0 && value.y >= 0); size = value; }
};
struct Play { Clip clip; };
struct Motion {
    float2 size{};
    void InsertKeyFrame(float, float2 value, int) { assert(value.x >= 0 && value.y >= 0); size = value; }
};
int main() {
    for (float shift : {-2.0f, 0.0f, 5.0f}) {
        const std::vector<Rect> characters{{shift, 0, 10, 20}, {shift+10, 0, 12, 20},
                                          {shift-1, 20, 9, 20}, {shift+8, 20, 11, 20}};
        for (double place : {0.0, 0.25, 1.0, 1.75, 2.0, 2.5, 3.0, 3.75}) {
            auto play = std::make_shared<Play>();
            auto whole = static_cast<size_t>(place);
            auto sameLine = LINE
            auto top = CLIP
            assert(play->clip.offset.x <= characters[first].X);
            assert(play->clip.offset.x + play->clip.size.x == edge(place));
            assert(play->clip.offset.y == characters[whole].Y);
            assert(play->clip.size.y == characters[whole].Height);
            Motion motion;
            float progress = 0.5f;
            int even = 0;
            for (float x : {edge(place), edge(static_cast<double>(last))}) {
                KEYFRAME
                assert(play->clip.offset.x + motion.size.x == x);
                assert(motion.size.y == height);
            }
        }
    }
}
"#.replace("LINE", line).replace("CLIP", clip).replace("KEYFRAME", keyframe);
    let dir = std::env::temp_dir().join(format!("bern-lit-clip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cpp = dir.join("clip.cpp");
    let binary = dir.join("clip");
    std::fs::write(&cpp, source).unwrap();
    let compiled = std::process::Command::new("c++")
        .args(["-std=c++20", "-Wall", "-Wextra", "-Werror"])
        .arg(cpp)
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
