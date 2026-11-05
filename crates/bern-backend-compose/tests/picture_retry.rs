//! Pictures from the network come after a failure, and an address that will
//! not give one is not asked again. Exercises the actual
//! `BernPictureTries` on this computer, against a local server that fails on
//! purpose, with the waits shortened. Compiling Kotlin needs Gradle, so it
//! does nothing where the Android tools are not installed.

#[path = "../../bern-target-api/tests/faulty_server/mod.rs"]
mod faulty_server;

use bern_backend_compose::toolchain;
use std::fs;
use std::process::Command;

const BUILD: &str = r#"
plugins {
    kotlin("jvm") version "{KOTLIN}"
    application
}
repositories { mavenCentral() }
dependencies { implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0") }
application { mainClass.set("dev.bern.runtime.PictureRetryKt") }
"#;

const SETTINGS: &str = r#"
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}
rootProject.name = "picture-retry"
"#;

const TESTS: &str = r#"
package dev.bern.runtime

import java.net.URL
import kotlin.system.exitProcess
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext

const val PNG = 0x89.toByte()

/// One try, as `BernPictures` makes it: an answer that is no picture is
/// refused.
fun attempt(url: String): BernTry<ByteArray> = when (val read = BernPictureTries.download(url)) {
    is BernTry.Came -> if (read.value.firstOrNull() == PNG) read else BernTry.Refused
    is BernTry.Failed -> read
}

fun expect(holds: Boolean, what: String) {
    if (!holds) {
        System.err.println("failed: $what")
        exitProcess(1)
    }
}

fun main(arguments: Array<String>): Unit = runBlocking(Dispatchers.Default) {
    val base = arguments[0]
    val silent = arguments[1]
    // A load waits as long as it takes, and a failure here would too.
    Thread {
        Thread.sleep(60_000)
        System.err.println("timed out")
        Runtime.getRuntime().halt(1)
    }.apply { isDaemon = true }.start()
    fun text(path: String) = URL(base + path).readText()
    fun asked(path: String) = text("/asked$path").toInt()
    /// How often `path` was asked for, once that has stopped changing.
    suspend fun settled(path: String): Int {
        var count = asked(path)
        var still = 0
        while (still < 4 || count == 0) {
            delay(250)
            val now = asked(path)
            still = if (now == count) still + 1 else 0
            count = now
        }
        return count
    }
    fun load(url: String): Deferred<ByteArray?> = async {
        BernPictureTries.until(url) { withContext(Dispatchers.IO) { attempt(url) } }
    }
    fun get(path: String) = load(base + path)
    // The system's first word on the network, which changes nothing.
    BernPictureTries.networkChanged(online = true)

    // A failure on the way, and the picture still comes.
    expect(get("/breaks/1").await() != null, "a broken connection is tried again")
    expect(asked("/breaks/1") >= 3, "breaks")
    expect(get("/busy/1").await() != null, "a busy server is tried again")
    expect(asked("/busy/1") == 3, "busy")

    // A connection that stalls reports nothing, and is given up on: the
    // server stays silent for 20 seconds, and a try may wait for 1.
    val stalled = System.currentTimeMillis()
    expect(get("/stalls/1").await() != null, "a silent try is given up on")
    expect(asked("/stalls/1") == 2, "stalls")
    expect(System.currentTimeMillis() - stalled < 10_000, "a silent try is given up on in time")

    // Nothing listens at first, as while a proxy starts.
    val starting = load("http://127.0.0.1:$silent/picture")
    delay(300)
    text("/listen/$silent")
    BernPictureTries.tryAgain()
    expect(starting.await() != null, "a refused connection is tried again")

    // Tries end, and a load rests until something changes.
    val down = get("/down/1")
    val tried = settled("/down/1")
    expect(tried >= 3, "tries before resting: $tried")
    BernPictureTries.tryAgain()
    expect(settled("/down/1") > tried, "the app coming to the front tries again")
    text("/heal/down/1")
    BernPictureTries.networkChanged(online = false)
    delay(50)
    BernPictureTries.networkChanged(online = true)
    expect(down.await() != null, "the network coming back tries again")

    // A view that goes away stops the tries, and one that appears again
    // starts over.
    val gone = get("/down/2")
    delay(10)
    gone.cancelAndJoin()
    val left = settled("/down/2")
    delay(300)
    expect(asked("/down/2") == left, "no tries for a view that went away")
    text("/heal/down/2")
    expect(get("/down/2").await() != null, "a view that appears again tries at once")

    // An answer that is no picture is not asked for again: not by the same
    // view, a new one or the app coming to the front.
    for (path in listOf("/gone/1", "/page/1")) {
        val first = get(path)
        expect(settled(path) == 1, path)
        first.cancelAndJoin()
        val second = get(path)
        BernPictureTries.tryAgain()
        expect(settled(path) == 1, path)
        // On another network it may have one after all.
        text("/heal$path")
        BernPictureTries.networkChanged(online = true)
        expect(second.await() != null, path)
        expect(asked(path) == 2, path)
    }

    // A file is looked for once, and looked for again by the next view.
    expect(BernPictureTries.until("file:///nothing") { BernTry.Absent as BernTry<ByteArray> } == null, "a missing file")
    exitProcess(0)
}
"#;

#[test]
fn pictures_come_after_failures_and_refusals_are_not_repeated() {
    let Some(gradle) = toolchain::gradle() else {
        eprintln!("no Gradle here: the Kotlin was not run");
        return;
    };
    let (address, silent) = faulty_server::start();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/picture-retry-compose");
    let sources = root.join("src/main/kotlin");
    fs::create_dir_all(&sources).unwrap();
    let tries = include_str!("../templates/BernPictureTries.kt");
    let waits = "listOf(1.0, 2.0, 4.0, 8.0, 16.0, 32.0)";
    assert!(tries.contains(waits), "{tries}");
    let silence = "silence = 15_000";
    assert!(tries.contains(silence), "{tries}");
    let tries = tries
        .replace(waits, "listOf(0.02, 0.02)")
        .replace(silence, "silence = 1_000");
    fs::write(sources.join("BernPictureTries.kt"), tries).unwrap();
    fs::write(sources.join("PictureRetry.kt"), TESTS).unwrap();
    fs::write(root.join("settings.gradle.kts"), SETTINGS).unwrap();
    fs::write(
        root.join("build.gradle.kts"),
        BUILD.replace("{KOTLIN}", bern_backend_compose::KOTLIN),
    )
    .unwrap();
    let mut command = Command::new(gradle);
    command
        .args(["--console=plain", "--quiet", "run"])
        .arg(format!("--args=http://{address} {silent}"))
        .current_dir(&root);
    if let Some(java) = toolchain::java_home() {
        command.env("JAVA_HOME", java);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
