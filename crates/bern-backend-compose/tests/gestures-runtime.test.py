"""Compile the actual gesture adapters and execute their pure runtime state.

Uses only already cached Kotlin/Compose jars and the installed Android SDK;
no device, emulator, Gradle downloads, or separate Cargo target directory.
"""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
import zipfile


class GesturesRuntime(unittest.TestCase):
    def test_touch_gestures_and_brightness(self):
        home = Path.home()
        cache = home / ".gradle/caches/modules-2/files-2.1"
        def latest(group, artifact, suffix="jar"):
            paths = list((cache / group / artifact).glob(f"*/*/*.{suffix}"))
            return max(paths, key=lambda p: tuple(map(int, re.findall(r"\d+", p.parts[-3])))) if paths else None
        compiler = latest("org.jetbrains.kotlin", "kotlin-compiler-embeddable")
        plugin = latest("org.jetbrains.kotlin", "kotlin-compose-compiler-plugin-embeddable")
        java_home = Path(os.environ.get("BERN_JAVA_HOME", "/Applications/Android Studio.app/Contents/jbr/Contents/Home"))
        java = java_home / "bin/java"
        sdk = Path(os.environ.get("ANDROID_HOME", str(home / "Library/Android/sdk")))
        android = sorted((sdk / "platforms").glob("android-*/android.jar"))
        if not compiler or not plugin or not java.exists() or not android:
            self.skipTest("cached Kotlin/Compose compiler and Android SDK required")
        target = os.environ.get("CARGO_TARGET_DIR")
        if not target:
            self.skipTest("set the shared CARGO_TARGET_DIR; this test never creates another target")
        base = Path(target) / "bern-test"
        base.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="touch-runtime-", dir=base) as folder:
            output = Path(folder)
            deps = []
            for group in ["org.jetbrains.kotlin", "org.jetbrains.kotlinx", "org.jetbrains", "androidx.annotation", "androidx.collection"]:
                for artifact in (cache / group).glob("*"):
                    jar = latest(group, artifact.name)
                    if jar:
                        deps.append(jar)
            compiler_cp = os.pathsep.join(map(str, [compiler, *deps]))
            cp = [android[-1], *deps]
            for group in cache.glob("androidx.*"):
                if not group.name.startswith(("androidx.compose.", "androidx.lifecycle", "androidx.savedstate")):
                    continue
                for artifact in group.iterdir():
                    jar = latest(group.name, artifact.name)
                    if jar:
                        cp.append(jar)
                    aar = latest(group.name, artifact.name, "aar")
                    if aar:
                        with zipfile.ZipFile(aar) as archive:
                            if "classes.jar" in archive.namelist():
                                jar = output / f"{group.name}-{artifact.name}.jar"
                                jar.write_bytes(archive.read("classes.jar"))
                                cp.append(jar)
            source = (Path(__file__).parent / "../templates/BernFeatures.kt").resolve().read_text()
            runtime = source[source.index("internal class BernSwipeTrack("):source.index("// MARK: - Rails")]
            imports = """
package dev.bern.runtime
import androidx.compose.runtime.*
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.*
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.compose.currentStateAsState
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
class BernWindowContext(val kindID: String = "main")
val LocalBernWindow = staticCompositionLocalOf { BernWindowContext() }
object BernHost { val sheets = emptyList<String>(); val primary = "main" }
"""
            main = """
fun main() {
    val horizontal = BernSwipeTrack(0.5, true, 300.0, 0.0, 1.0, 10f)
    check(horizontal.move(9f, 0f) == null && !horizontal.accepted)
    check(horizontal.move(60f, 2f) == 0.7 && horizontal.accepted)
    check(horizontal.move(90f, 900f) == 0.8)
    check(horizontal.move(1000f, 0f) == 1.0)
    val wrong = BernSwipeTrack(0.5, true, 300.0, 0.0, 1.0, 10f)
    check(wrong.move(20f, 20f) == null && wrong.rejected)
    check(wrong.move(300f, 0f) == null)
    val vertical = BernSwipeTrack(0.5, false, 300.0, 0.0, 1.0, 10f)
    check(vertical.move(0f, -60f) == 0.7)
    check(vertical.move(0f, 300f) == 0.0)
    var draftAtEnd = 0.5
    var dragging = true
    val completion = mutableListOf<String>()
    bernCommitSwipe(
        { draftAtEnd = 0.8 },
        { completion.add("action:$draftAtEnd:$dragging") },
        { completion.add("commit:$draftAtEnd:$dragging") },
        { dragging = false; draftAtEnd = 0.1; completion.add("clear") },
    )
    check(completion == listOf("action:0.8:true", "commit:0.8:true", "clear"))
    check(!dragging && draftAtEnd == 0.1)
    dragging = true
    completion.clear()
    try {
        bernCommitSwipe({}, { error("action failed") }, { completion.add("commit") }, { dragging = false; completion.add("clear") })
    } catch (_: IllegalStateException) {}
    check(!dragging && completion == listOf("clear"))
    var override = -1f
    var effective = 0.4
    var draft = 0.0
    var available = false
    val writes = mutableListOf<Float>()
    val overrides = BernBrightnessOverrides({ override }, { effective }, { override = it; writes.add(it) })
    val lease = BernBrightnessLease(overrides, { draft = it }, { available = it })
    lease.resume()
    check(available && draft == 0.4 && writes.isEmpty())
    lease.update(draft)
    lease.update(Double.NaN)
    lease.update(Double.POSITIVE_INFINITY)
    check(writes.isEmpty())
    lease.update(0.7)
    lease.update(0.7)
    check(writes == listOf(0.7f))
    lease.pause()
    check(!available && override == -1f)
    lease.pause()
    check(writes == listOf(0.7f, -1f))
    lease.update(0.1)
    check(override == -1f)
    effective = 0.8
    lease.resume()
    check(draft == 0.8 && writes.size == 2)
    lease.update(2.0)
    check(override == 1f)
    lease.pause()
    override = 0.6f
    lease.resume()
    check(draft == 0.6f.toDouble())
    lease.update(-0.2)
    check(override == 0f)
    lease.pause()
    check(override == 0.6f)
    override = Float.NaN
    lease.resume()
    check(!available)
    // Old-owner-first disposal must not overwrite the current owner's
    // override; the final owner restores the pre-stack inherited sentinel.
    override = -1f
    writes.clear()
    val older = BernBrightnessLease(overrides, {}, {})
    var newestDraft = 0.0
    val newer = BernBrightnessLease(overrides, { newestDraft = it }, {})
    older.resume()
    older.update(0.3)
    newer.resume()
    check(newestDraft == 0.3f.toDouble() && writes == listOf(0.3f))
    newer.update(0.8)
    val beforeOldLeaves = writes.toList()
    older.pause()
    check(override == 0.8f && writes == beforeOldLeaves)
    newer.pause()
    check(override == -1f && writes.last() == -1f)
    // New-owner-first disposal hands control to the remaining owner, even
    // if that owner changed its desired value while it was covered.
    override = 0.2f
    writes.clear()
    older.resume()
    older.update(0.4)
    newer.resume()
    newer.update(0.9)
    older.update(0.6)
    check(override == 0.9f && writes == listOf(0.4f, 0.9f))
    newer.pause()
    check(override == 0.6f)
    older.pause()
    check(override == 0.2f)
    // Another window has its own stack and restoration baseline.
    var otherOverride = -1f
    val other = BernBrightnessLease(BernBrightnessOverrides({ otherOverride }, { 0.5 }, { otherOverride = it }), {}, {})
    older.resume()
    older.update(0.7)
    other.resume()
    other.update(0.9)
    older.pause()
    check(override == 0.2f && otherOverride == 0.9f)
    other.pause()
    check(otherOverride == -1f)
    println("touch runtime: axis locking, fixed start, bounds, commit-before-clear, shared-window brightness lifecycle/finite checks passed")
}
"""
            path = output / "Touch.kt"
            path.write_text(imports + runtime + main)
            classes = output / "classes"
            args = [str(java), "-cp", compiler_cp, "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", "-no-stdlib", "-no-reflect", "-jvm-target", "17", "-Xplugin=" + str(plugin), "-classpath", os.pathsep.join(map(str, cp)), "-d", str(classes), str(path)]
            result = subprocess.run(args, capture_output=True, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            result = subprocess.run([str(java), "-cp", os.pathsep.join(map(str, [classes, *cp])), "dev.bern.runtime.TouchKt"], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            print(result.stdout.strip())


if __name__ == "__main__":
    unittest.main()
