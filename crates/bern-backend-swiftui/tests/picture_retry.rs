//! Pictures from the network come after a failure, and an address that will
//! not give one is not asked again. Exercises the actual `BernPictures`
//! against a local server that fails on purpose, with the waits shortened;
//! no application engine is needed.
#[cfg(target_os = "macos")]
#[path = "../../bern-target-api/tests/faulty_server/mod.rs"]
mod faulty_server;

#[cfg(target_os = "macos")]
#[test]
fn pictures_come_after_failures_and_refusals_are_not_repeated() {
    use std::{fs, process::Command};

    let (address, silent) = faulty_server::start();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bern-test/picture-retry");
    fs::create_dir_all(&root).unwrap();
    let runtime = include_str!("../templates/BernRuntime.swift");
    let start = runtime
        .find("/// Pictures loaded from files and http(s) URLs")
        .unwrap();
    let end = runtime.find("// MARK: - Files").unwrap();
    let pictures = &runtime[start..end];
    let waits = "[1, 2, 4, 8, 16, 32]";
    assert!(pictures.contains(waits), "{pictures}");
    let silence = "silence: TimeInterval = 15";
    assert!(pictures.contains(silence), "{pictures}");
    let network = "private func networkChanged";
    assert!(pictures.contains(network), "{pictures}");
    // This fixture drives network changes itself. Real path callbacks would
    // invalidate refusals between assertions even when only loopback is used.
    let monitor =
        r#"network.start(queue: DispatchQueue(label: "dev.bern.pictures.network", qos: .utility))"#;
    assert!(pictures.contains(monitor), "{pictures}");
    let pictures = pictures
        .replace(monitor, "")
        .replace(waits, "[0.02, 0.02]")
        .replace(silence, "silence: TimeInterval = 1")
        .replace(network, "func networkChanged");
    let prelude = r#"
import AppKit
import ImageIO
import Network
import SwiftUI
enum BernImageFit { case contain, cover, fill }
enum BernColor { case fixed(light: UInt32, dark: UInt32) }
// No engine here, so it serves nothing.
struct BernBuffer { var ptr: UnsafeMutablePointer<UInt8>! = nil; var len = 0 }
func bern_files_served(_ url: UnsafePointer<UInt8>?, _ len: Int) -> UInt8 { 0 }
func bern_files_read(_ url: UnsafePointer<UInt8>?, _ len: Int) -> BernBuffer { BernBuffer() }
func bern_buffer_free(_ buffer: BernBuffer) {}
enum BernImage {
    struct Picture: @unchecked Sendable { let image: CGImage }
    nonisolated static func sampledPixels(_ source: CGImageSource, width: CGFloat?, height: CGFloat?, fit: BernImageFit, scale: CGFloat) -> Int? { nil }
    nonisolated static func decode(_ source: CGImageSource, pixels: Int? = nil) -> Picture? {
        CGImageSourceCreateImageAtIndex(source, 0, nil).map(Picture.init)
    }
}
"#;
    let tests = r#"
@main struct PictureRetryTest {
    @MainActor static func main() async {
        let base = CommandLine.arguments[1]
        let silent = CommandLine.arguments[2]
        let pictures = BernPictures.shared
        // A load waits as long as it takes, and a failure here would too.
        Task.detached {
            try await Task.sleep(for: .seconds(60))
            fatalError("timed out")
        }
        func text(_ path: String) async -> String {
            let data = try! await URLSession.shared.data(from: URL(string: base + path)!).0
            return String(decoding: data, as: UTF8.self)
        }
        func expect(_ holds: Bool, _ what: String = "", line: Int = #line) {
            if !holds { fatalError("line \(line): \(what)") }
        }
        func asked(_ path: String) async -> Int { Int(await text("/asked" + path))! }
        func settle(_ seconds: Double = 0.3) async { try! await Task.sleep(for: .seconds(seconds)) }
        /// How often `path` was asked for, once that has stopped changing: a
        /// busy machine may take its time, and the count must not be early.
        func settled(_ path: String) async -> Int {
            var count = await asked(path), still = 0
            while still < 4 || count == 0 {
                await settle(0.25)
                let now = await asked(path)
                still = now == count ? still + 1 : 0
                count = now
            }
            return count
        }
        func load(_ path: String) -> Task<BernImage.Picture?, Never> {
            Task { await pictures.load(BernPictures.Request(url: base + path)) }
        }
        func toFront() { NotificationCenter.default.post(name: NSApplication.didBecomeActiveNotification, object: nil) }
        // The initial network event is explicit, just like the later changes.
        pictures.networkChanged(online: true)

        // A failure on the way, and the picture still comes.
        expect(await load("/breaks/1").value != nil, "a broken connection is tried again")
        expect(await asked("/breaks/1") >= 3)
        expect(await load("/busy/1").value != nil, "a busy server is tried again")
        expect(await asked("/busy/1") == 3)
        expect(await load("/busy/1").value != nil)
        expect(await asked("/busy/1") == 3, "a picture that came is kept")

        // A connection that stalls reports nothing, and is given up on.
        expect(await load("/stalls/1").value != nil, "a silent try is given up on")
        expect(await asked("/stalls/1") == 2)

        // Nothing listens at first, as while a proxy starts.
        let starting = Task {
            await pictures.load(BernPictures.Request(url: "http://127.0.0.1:\(silent)/picture"))
        }
        await settle()
        _ = await text("/listen/\(silent)")
        toFront()
        expect(await starting.value != nil, "a refused connection is tried again")

        // Tries end, and a load rests until something changes.
        let down = load("/down/1")
        let tried = await settled("/down/1")
        expect(tried >= 3, "\(tried)")
        toFront()
        expect(await settled("/down/1") > tried, "the app coming to the front tries again")
        _ = await text("/heal/down/1")
        pictures.networkChanged(online: false)
        await settle(0.05)
        pictures.networkChanged(online: true)
        expect(await down.value != nil, "the network coming back tries again")

        // A view that goes away stops the tries, and one that appears
        // again starts over.
        let gone = load("/down/2")
        await settle(0.01)
        gone.cancel()
        expect(await gone.value == nil)
        let left = await settled("/down/2")
        await settle()
        expect(await asked("/down/2") == left, "no tries for a view that went away")
        _ = await text("/heal/down/2")
        expect(await load("/down/2").value != nil, "a view that appears again tries at once")

        // An answer that is no picture is not asked for again: not by the
        // same view, a new one or the app coming to the front.
        for path in ["/gone/1", "/page/1"] {
            let first = load(path)
            let observed = await settled(path)
            expect(observed == 1, "\(path): count=\(observed)")
            first.cancel()
            expect(await first.value == nil, path)
            let second = load(path)
            toFront()
            let resumed = await settled(path)
            expect(resumed == 1, "\(path): foreground count=\(resumed)")
            // On another network it may have one after all.
            _ = await text("/heal" + path)
            pictures.networkChanged(online: true)
            expect(await second.value != nil, path)
            expect(await asked(path) == 2, path)
        }

        // A file is looked for once, and looked for again by the next view.
        let missing = "file:///nothing/here.png"
        expect(await pictures.load(BernPictures.Request(url: missing)) == nil)

        // Fills ask on every drawing, which adds no tries.
        for _ in 0..<40 {
            _ = pictures.average(base + "/down/3")
            await settle(0.01)
        }
        let filled = await settled("/down/3")
        for _ in 0..<40 {
            _ = pictures.average(base + "/down/3")
            await settle(0.01)
        }
        expect(await asked("/down/3") == filled, "drawing does not try again")
        _ = await text("/heal/down/3")
        pictures.networkChanged(online: true)
        while pictures.average(base + "/down/3") == nil { await settle(0.05) }
    }
}
"#;
    let source = root.join("main.swift");
    fs::write(&source, format!("{prelude}{pictures}{tests}")).unwrap();
    let binary = root.join("picture-retry");
    let result = Command::new("xcrun")
        .args(["swiftc", "-swift-version", "6", "-parse-as-library"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(&binary)
        .arg(format!("http://{address}"))
        .arg(silent.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
