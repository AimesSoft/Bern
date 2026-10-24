// Animated SVG is an isolated, transparent document. WebKit is loaded
// only by applications declaring Svg; no app command handlers are exposed.
import WebKit

private let bernSvgPrefix = /*bern:svg-prefix*/
private let bernSvgSuffix = /*bern:svg-suffix*/

struct BernSvg: View {
    let name: String
    let label: String?
    let width: CGFloat
    let height: CGFloat
    var fit: String = "contain"
    var playing: Bool = true
    var speed: Double = 1
    var restart: Double = 0
    var interactive: Bool = true
    var data: String = ""
    var externalLinks: Bool = false
    @Environment(\.colorScheme) private var scheme
    @Environment(\.scenePhase) private var phase
    @State private var visible = false

    var body: some View {
        BernSvgDocument(name: name, state: [
            "playing": playing, "visible": visible && phase == .active,
            "speed": speed.isFinite && speed > 0 ? speed : 1,
            "restart": restart.isFinite ? restart : 0,
            "interactive": interactive, "fit": fit, "data": data, "externalLinks": externalLinks,
            "scheme": scheme == .dark ? "dark" : "light"
        ])
        .frame(width: max(0, width), height: max(0, height))
        .clipped()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label ?? "")
        .accessibilityHidden(label == nil)
        .onAppear { visible = true }
        .onDisappear { visible = false }
    }
}

@MainActor
private final class BernSvgCoordinator: NSObject, WKNavigationDelegate {
    var name = ""
    var state: [String: Any] = [:]
    var loaded = false
    var lastState: Data?

    func apply(_ view: WKWebView) {
        guard loaded, let data = try? JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]), data != lastState,
              let json = String(data: data, encoding: .utf8) else { return }
        lastState = data
        view.evaluateJavaScript("window.bernSvg?.update(\(json))", completionHandler: nil)
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        apply(webView)
    }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction) async -> WKNavigationActionPolicy {
        if state["externalLinks"] as? Bool == true, action.navigationType == .linkActivated,
           let url = action.request.url, url.scheme == "https", url.host != nil {
            #if os(macOS)
            NSWorkspace.shared.open(url)
            #else
            // In this async method the plain call would be the async overload.
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
            #endif
        }
        // The document itself never navigates away from the bundled source.
        return !loaded && action.request.url?.absoluteString == "about:blank" ? .allow : .cancel
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        NSLog("Bern SVG %@: %@", name, error.localizedDescription)
    }
}

@MainActor
private struct BernSvgDocument {
    let name: String
    let state: [String: Any]

    func makeCoordinator() -> BernSvgCoordinator { BernSvgCoordinator() }
    func makeView(_ coordinator: BernSvgCoordinator) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.navigationDelegate = coordinator
        #if os(macOS)
        view.setValue(false, forKey: "drawsBackground")
        #else
        view.isOpaque = false
        view.backgroundColor = .clear
        view.scrollView.backgroundColor = .clear
        view.scrollView.isScrollEnabled = false
        #endif
        return view
    }
    func update(_ view: WKWebView, _ coordinator: BernSvgCoordinator) {
        coordinator.state = state
        if coordinator.name != name {
            coordinator.name = name
            coordinator.loaded = false
            coordinator.lastState = nil
            guard let root = Bundle.main.resourceURL,
                  let source = try? String(contentsOf: root.appendingPathComponent(name), encoding: .utf8) else {
                NSLog("Bern SVG: unable to load %@", name)
                return
            }
            view.loadHTMLString(bernSvgPrefix + source + bernSvgSuffix, baseURL: nil)
        } else { coordinator.apply(view) }
    }
}

#if os(macOS)
extension BernSvgDocument: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { makeView(context.coordinator) }
    func updateNSView(_ view: WKWebView, context: Context) { update(view, context.coordinator) }
    static func dismantleNSView(_ view: WKWebView, coordinator: BernSvgCoordinator) {
        view.stopLoading()
        view.navigationDelegate = nil
        view.loadHTMLString("", baseURL: nil)
    }
}
#else
extension BernSvgDocument: UIViewRepresentable {
    func makeUIView(context: Context) -> WKWebView { makeView(context.coordinator) }
    func updateUIView(_ view: WKWebView, context: Context) { update(view, context.coordinator) }
    static func dismantleUIView(_ view: WKWebView, coordinator: BernSvgCoordinator) {
        view.stopLoading()
        view.navigationDelegate = nil
        view.loadHTMLString("", baseURL: nil)
    }
}
#endif
