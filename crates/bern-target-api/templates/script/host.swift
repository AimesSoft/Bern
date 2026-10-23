
import WebKit

@MainActor
private final class BernScriptHost: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
    static let shared = BernScriptHost()
    private var view: WKWebView?
    private var timer: Timer?
    private var ready = false
    private var pending: [String] = []
    func start() {
        guard timer == nil else { return }
        bern_script_start()
        timer = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) { _ in
            Task { @MainActor in BernScriptHost.shared.pump() }
        }
    }
    private func open() {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.add(self, name: "bernScript")
        let view = WKWebView(frame: .zero, configuration: configuration)
        self.view = view
        view.navigationDelegate = self
        view.loadHTMLString(/*bern:script-html*/, baseURL: nil)
    }
    func stop() {
        timer?.invalidate(); timer = nil
        view?.configuration.userContentController.removeScriptMessageHandler(forName: "bernScript")
        view?.stopLoading(); view = nil; ready = false; pending.removeAll()
        bern_script_stop()
    }
    private func pump() {
        let buffer = bern_script_poll(ProcessInfo.processInfo.systemUptime * 1000)
        if let ptr = buffer.ptr, buffer.len > 0 {
            pending.append(String(decoding: UnsafeBufferPointer(start: ptr, count: Int(buffer.len)), as: UTF8.self))
        }
        bern_buffer_free(buffer)
        if view == nil && !pending.isEmpty { open() }
        guard ready, let view else { return }
        let batches = pending; pending.removeAll()
        for batch in batches {
            view.evaluateJavaScript("bernScriptDispatch(\(batch))") { _, error in
                if error != nil { BernScriptHost.shared.stop() }
            }
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { ready = true }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { stop() }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { stop() }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { stop() }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, let json = message.body as? String else { return }
        Array(json.utf8).withUnsafeBufferPointer { bern_script_complete($0.baseAddress, $0.count) }
    }
}
