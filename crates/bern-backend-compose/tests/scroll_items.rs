//! Run the generated scroll state's bookkeeping on the JVM. Small layout
//! and state stand-ins keep this independent of an Android device; disposal
//! wiring is checked separately and still needs an actual Compose run.
use bern_backend_compose::toolchain;
use std::{fs, process::Command};

const FEATURES: &str = include_str!("../templates/BernFeatures.kt");

#[test]
fn scroll_items_leave_with_their_composition() {
    let item = FEATURES
        .split("fun Modifier.bernScrollItem(key: Any)")
        .nth(1)
        .unwrap();
    let item = item.split("/// Follows scrolling").next().unwrap();
    assert!(item.contains("DisposableEffect(state, key)"));
    assert!(item.contains("onDispose { state.remove(key) }"));
}

#[test]
fn removed_and_resized_items_do_not_take_part_in_snapping() {
    let Some(gradle) = toolchain::gradle() else {
        eprintln!("no Gradle here: scroll bookkeeping was not run on the JVM");
        return;
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/scroll-items-compose");
    let sources = root.join("src/main/kotlin");
    fs::create_dir_all(&sources).unwrap();
    let state = FEATURES
        .split("class BernScrollState")
        .nth(1)
        .unwrap()
        .split("val LocalBernScroll")
        .next()
        .unwrap();
    let state = format!("class BernScrollState{state}")
        .replace("androidx.compose.ui.geometry.Offset.Zero", "Offset.Zero");
    let fling = FEATURES
        .split("private class BernSnapFling")
        .nth(1)
        .unwrap()
        .split("/// A scroll view whose items")
        .next()
        .unwrap();
    let fling = format!("private class BernSnapFling{fling}");
    fs::write(
        sources.join("ScrollItems.kt"),
        format!("{STUBS}\n{state}\n{fling}\n{TESTS}"),
    )
    .unwrap();
    fs::write(root.join("settings.gradle.kts"), "pluginManagement { repositories { gradlePluginPortal(); mavenCentral() } }\nrootProject.name = \"scroll-items\"\n").unwrap();
    fs::write(
        root.join("build.gradle.kts"),
        format!(
            r#"
plugins {{ kotlin("jvm") version "{}"; application }}
repositories {{ mavenCentral() }}
application {{ mainClass.set("ScrollItemsKt") }}
"#,
            bern_backend_compose::KOTLIN
        ),
    )
    .unwrap();
    let mut command = Command::new(gradle);
    command
        .args(["--console=plain", "--quiet", "run"])
        .current_dir(root);
    if let Some(java) = toolchain::java_home() {
        command.env("JAVA_HOME", java);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const STUBS: &str = r#"
import kotlin.reflect.KProperty
class State<T>(var value: T) {
    operator fun getValue(owner: Any?, property: KProperty<*>) = value
    operator fun setValue(owner: Any?, property: KProperty<*>, next: T) { value = next }
}
fun <T> mutableStateOf(value: T) = State(value)
fun <K,V> mutableStateMapOf() = mutableMapOf<K,V>()
class ScrollState(var value: Int) {
    var maxValue: Int = 10000
    fun dispatchRawDelta(delta: Float): Float {
        val old = value
        value = (value + delta).toInt().coerceIn(0, maxValue)
        return (value - old).toFloat()
    }
}
interface ScrollScope { fun scrollBy(pixels: Float): Float }
interface FlingBehavior { suspend fun ScrollScope.performFling(initialVelocity: Float): Float }
class DecayAnimationSpec<T> {
    fun calculateTargetValue(start: Float, velocity: Float) = start + velocity
}
suspend fun animate(start: Float, target: Float, initialVelocity: Float, block: (Float, Float) -> Unit) {
    for (value in listOf(start, start + initialVelocity, (start + target) / 2f, target)) block(value, 0f)
}
data class Offset(val x: Float, val y: Float) { companion object { val Zero = Offset(0f,0f) } }
data class Size(val width: Float, val height: Float) { fun toSize() = this }
data class Rect(val left: Float, val top: Float, val right: Float, val bottom: Float) {
    constructor(at: Offset, size: Size): this(at.x, at.y, at.x + size.width, at.y + size.height)
    val width get() = right - left
    val height get() = bottom - top
    companion object { val Zero = Rect(0f,0f,0f,0f) }
}
class LayoutCoordinates(val at: Offset, val size: Size, var isAttached: Boolean = true) {
    fun localPositionOf(item: LayoutCoordinates, offset: Offset) = Offset(item.at.x - at.x + offset.x, item.at.y - at.y + offset.y)
}
"#;

const TESTS: &str = r#"
suspend fun main() {
    for (horizontal in listOf(true, false)) {
        val state = BernScrollState(horizontal, false)
        state.viewport = Rect(0f, 0f, 448f, 448f)
        state.content = LayoutCoordinates(Offset.Zero, Size(2000f,2000f))
        fun put(key: String, start: Float, extent: Float) {
            state.report(key, LayoutCoordinates(
                if (horizontal) Offset(start,0f) else Offset(0f,start),
                if (horizontal) Size(extent,100f) else Size(100f,extent)))
        }
        put("old-first",0f,400f)
        put("old-last",1000f,400f)
        state.remove("old-first")
        state.remove("old-last")
        put("new-first",0f,220f)
        put("new-last",250f,220f)
        check(!state.known("old-last"))
        check(state.offset("old-last") == null)
        check(state.nearest(1000f) == "new-last")
        check(state.largest == 220f)
        put("new-last",250f,120f)
        check(state.largest == 220f)
        state.remove("new-first")
        check(state.largest == 120f)
        put("new-last",250f,80f)
        check(state.largest == 80f)
        state.remove("new-last")
        check(state.items.isEmpty() && state.largest == 0f)
        check(state.nearest() == null)
        state.remove("already-gone")
        check(state.largest == 0f)
        put("first",16f,220f)
        put("last",516f,220f)
        state.scroll.maxValue = 532
        for ((at, velocity) in listOf(532 to 0f, 400 to 1000f, 0 to -1000f)) {
            state.scroll.value = at
            val expected = state.nearest(at + velocity)?.let(state::offset)!!.toInt()
            val fling = BernSnapFling(state, DecayAnimationSpec<Float>())
            // Edge stretch can consume every pixel without changing the
            // content's scroll offset, as on an actual Android boundary.
            val stretched = object : ScrollScope {
                override fun scrollBy(pixels: Float) = pixels
            }
            with(fling) { with(stretched) { performFling(velocity) } }
            check(state.scroll.value == expected) { "snap ended at ${state.scroll.value}, expected $expected" }
        }
    }
    println("replacement, removal, resize, empty state and edge-stretch snapping passed on both axes")
}
"#;
