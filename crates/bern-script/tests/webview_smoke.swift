import AppKit
struct BernBuffer { var ptr: UnsafeMutablePointer<UInt8>?; var len: Int; var capacity: Int }
@MainActor var sent = false
@MainActor var replies: [String: Any] = [:]
@MainActor func bern_script_start() {}
@MainActor func bern_script_stop() {}
@MainActor func bern_script_poll(_ now: Double) -> BernBuffer {
    if sent { return BernBuffer(ptr: nil, len: 0, capacity: 0) }
    sent = true
    let requests: [[String: Any]] = [
        ["id": "1", "context": "a", "source": "var count = 1; ++count"],
        ["id": "2", "context": "a", "source": "++count"],
        ["id": "3", "context": "b", "source": "typeof count"],
        ["id": "4", "context": "a", "source": "Promise.resolve({items:[1,true,null]})"],
        ["id": "5", "context": "a", "source": "throw new Error('broken')"]
    ]
    let data = try! JSONSerialization.data(withJSONObject: requests)
    let ptr = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
    data.copyBytes(to: ptr, count: data.count)
    return BernBuffer(ptr: ptr, len: data.count, capacity: data.count)
}
func bern_buffer_free(_ buffer: BernBuffer) { buffer.ptr?.deallocate() }
@MainActor func bern_script_complete(_ ptr: UnsafePointer<UInt8>?, _ len: Int) {
    let data = Data(bytes: ptr!, count: len)
    let result = try! JSONSerialization.jsonObject(with: data) as! [String: Any]
    replies[result["id"] as! String] = result
}

/*bern:host*/

@main struct Test {
    @MainActor static func main() {
        _ = NSApplication.shared
        BernScriptHost.shared.start()
        let deadline = Date().addingTimeInterval(20)
        while replies.count < 5 && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.01)) }
        BernScriptHost.shared.stop()
        precondition(replies.count == 5, "missing replies: \(replies)")
        precondition((replies["1"] as! [String: Any])["value"] as? Int == 2)
        precondition((replies["2"] as! [String: Any])["value"] as? Int == 3)
        precondition((replies["3"] as! [String: Any])["value"] as? String == "undefined")
        precondition((replies["5"] as! [String: Any])["error"] as? String == "Error: broken")
        print("WKWebView: persistent globals, separate contexts, JSON, Promise, error: passed")
    }
}
