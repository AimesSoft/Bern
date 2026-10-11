// Measure a desktop app on macOS: the time from starting its executable
// until its first window is on screen, and its memory once it has
// settled. Memory is the physical footprint (what Activity Monitor shows)
// of the app and of every process it brought up: its child processes
// (Electron's helpers) and the WebKit processes that started with it
// (the pages of Tauri and Dioxus run in those).
//
//   swiftc -O measure.swift -o measure
//   ./measure <executable> [runs]
//
// Prints JSON: {"launch_s": [...], "memory_bytes": [...]}, one entry per
// run after an unmeasured first run that warms the caches.

import AppKit
import CoreGraphics
import Foundation

func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1e9 }

/// Every process: pid, parent pid and command.
func processes() -> [(pid: Int32, parent: Int32, command: String)] {
    let ps = Process()
    ps.executableURL = URL(fileURLWithPath: "/bin/ps")
    ps.arguments = ["-axo", "pid=,ppid=,comm="]
    let pipe = Pipe()
    ps.standardOutput = pipe
    try! ps.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    ps.waitUntilExit()
    return String(decoding: data, as: UTF8.self).split(separator: "\n").compactMap { line in
        let parts = line.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: true)
        guard parts.count == 3, let pid = Int32(parts[0]), let parent = Int32(parts[1]) else { return nil }
        return (pid, parent, String(parts[2]))
    }
}

/// Physical footprint of one process, in bytes.
func footprint(_ pid: Int32) -> Double {
    let tool = Process()
    tool.executableURL = URL(fileURLWithPath: "/usr/bin/footprint")
    tool.arguments = ["-p", String(pid)]
    let pipe = Pipe()
    tool.standardOutput = pipe
    tool.standardError = FileHandle.nullDevice
    guard (try? tool.run()) != nil else { return 0 }
    let text = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
    tool.waitUntilExit()
    // "phys_footprint: 19 MB"
    guard let line = text.split(separator: "\n").first(where: { $0.contains("phys_footprint:") }) else { return 0 }
    let words = line.split(separator: " ")
    guard let index = words.firstIndex(where: { $0.hasSuffix("phys_footprint:") }), index + 2 < words.count,
          let value = Double(words[index + 1]) else { return 0 }
    switch words[index + 2] {
    case "GB": return value * 1024 * 1024 * 1024
    case "MB": return value * 1024 * 1024
    case "KB": return value * 1024
    default: return value
    }
}

func onScreen(_ pid: Int32) -> Bool {
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    return windows.contains { window in
        (window[kCGWindowOwnerPID as String] as? Int32) == pid && (window[kCGWindowLayer as String] as? Int) == 0
    }
}

func measure(_ path: String) -> (launch: Double, memory: Double)? {
    let before = Set(processes().map(\.pid))
    let app = Process()
    app.executableURL = URL(fileURLWithPath: path)
    app.currentDirectoryURL = URL(fileURLWithPath: path).deletingLastPathComponent()
    app.standardOutput = FileHandle.nullDevice
    app.standardError = FileHandle.nullDevice
    let start = now()
    guard (try? app.run()) != nil else { return nil }
    let pid = app.processIdentifier
    var launch: Double?
    while launch == nil, now() - start < 60 {
        if onScreen(pid) { launch = now() - start } else { usleep(2000) }
    }
    // Settle, then count the app and what it brought up.
    sleep(4)
    let all = processes()
    var family: Set<Int32> = [pid]
    var grew = true
    while grew {
        grew = false
        for process in all where family.contains(process.parent) && !family.contains(process.pid) {
            family.insert(process.pid)
            grew = true
        }
    }
    for process in all where !before.contains(process.pid) && process.command.contains("com.apple.WebKit") {
        family.insert(process.pid)
    }
    let memory = family.map(footprint).reduce(0, +)
    app.terminate()
    app.waitUntilExit()
    for process in all where family.contains(process.pid) && process.pid != pid { kill(process.pid, SIGTERM) }
    sleep(2)
    guard let launch else { return nil }
    return (launch, memory)
}

let arguments = CommandLine.arguments
guard arguments.count >= 2 else {
    FileHandle.standardError.write("usage: measure <executable> [runs]\n".data(using: .utf8)!)
    exit(2)
}
let runs = arguments.count > 2 ? Int(arguments[2]) ?? 5 : 5
_ = measure(arguments[1])
var launches: [Double] = []
var memories: [Double] = []
for _ in 0..<runs {
    guard let result = measure(arguments[1]) else {
        FileHandle.standardError.write("no window within 60 s\n".data(using: .utf8)!)
        exit(1)
    }
    launches.append(result.launch)
    memories.append(result.memory)
}
let json = try! JSONSerialization.data(withJSONObject: ["launch_s": launches, "memory_bytes": memories])
print(String(decoding: json, as: UTF8.self))
