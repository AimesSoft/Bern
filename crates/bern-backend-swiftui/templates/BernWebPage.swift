// `WebPage`: embedded browser view, included only when the application
// declares `WebPage`.
import WebKit

/// A preference for the browsing environment, independent of a custom UA.
enum BernWebContentMode {
    case auto, mobile, desktop
}

/// `WebPage`: a page of the web, signed in with the cookies of a file. Its
/// session is its own and goes with it.
struct BernWebPage: View {
    let url: String
    let cookiesFile: String
    let cookieDomain: String
    let userAgent: String
    let contentMode: BernWebContentMode
    let closeOn: String
    let label: String?
    let close: () -> Void

    var body: some View {
        BernWebPageView(url: url, cookiesFile: cookiesFile, cookieDomain: cookieDomain, userAgent: userAgent, contentMode: contentMode, closeOn: closeOn, close: close)
            .accessibilityLabel(label ?? "")
    }
}

/// Where a page's press on `close_on` comes back to the app.
private let bernWebPageClose = "bern-web-page"

/// What listens in the page for a press on `selector`, before the page's own
/// handlers, which may do nothing outside their own app.
private func bernWebCloseScript(_ selector: String) -> String {
    let literal = String(data: (try? JSONEncoder().encode(selector)) ?? Data(), encoding: .utf8) ?? "\"\""
    return "document.addEventListener('click',function(e){var t=e.target;if(t&&t.closest&&t.closest(\(literal))){e.stopPropagation();e.preventDefault();location.href='\(bernWebPageClose)://close'}},true);"
}

/// The cookies of the file: `name=value; name=value`, attributes left out.
private func bernWebCookies(file: String, domain: String) -> [HTTPCookie] {
    guard !file.isEmpty, !domain.isEmpty, let text = try? String(contentsOfFile: file, encoding: .utf8) else { return [] }
    let attributes: Set<String> = ["path", "domain", "expires", "max-age", "secure", "httponly", "samesite"]
    return text.split(separator: ";").compactMap { part in
        let pair = part.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: "=", maxSplits: 1).map(String.init)
        guard pair.count == 2, !attributes.contains(pair[0].lowercased()) else { return nil }
        return HTTPCookie(properties: [.name: pair[0], .value: pair[1], .domain: domain, .path: "/", .secure: "TRUE"])
    }
}

@MainActor
private final class BernWebPageCoordinator: NSObject, WKUIDelegate, WKNavigationDelegate {
    var url = ""
    var close: () -> Void = {}

    /// The page's own schemes are for its app: only the web and a press on
    /// `close_on` go on.
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction) async -> WKNavigationActionPolicy {
        guard let scheme = action.request.url?.scheme else { return .allow }
        if scheme == bernWebPageClose { close() }
        return ["http", "https", "about", "blob", "data"].contains(scheme) ? .allow : .cancel
    }

    /// What opens a window of its own opens here.
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if action.targetFrame == nil { webView.load(action.request) }
        return nil
    }
}

@MainActor
private struct BernWebPageView {
    let url: String
    let cookiesFile: String
    let cookieDomain: String
    let userAgent: String
    let contentMode: BernWebContentMode
    let closeOn: String
    let close: () -> Void

    func makeCoordinator() -> BernWebPageCoordinator { BernWebPageCoordinator() }
    func makeView(_ coordinator: BernWebPageCoordinator) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        #if os(iOS)
        // A custom UA alone does not change iPad's desktop environment.
        // Leave the recommended mode untouched unless explicitly requested.
        switch contentMode {
        case .auto: break
        case .mobile: configuration.defaultWebpagePreferences.preferredContentMode = .mobile
        case .desktop: configuration.defaultWebpagePreferences.preferredContentMode = .desktop
        }
        #endif
        if !closeOn.isEmpty {
            configuration.userContentController.addUserScript(WKUserScript(
                source: bernWebCloseScript(closeOn), injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        let view = WKWebView(frame: .zero, configuration: configuration)
        if !userAgent.isEmpty { view.customUserAgent = userAgent }
        view.uiDelegate = coordinator
        view.navigationDelegate = coordinator
        return view
    }
    func update(_ view: WKWebView, _ coordinator: BernWebPageCoordinator) {
        coordinator.close = close
        guard coordinator.url != url, let address = URL(string: url) else { return }
        coordinator.url = url
        let store = view.configuration.websiteDataStore.httpCookieStore
        let group = DispatchGroup()
        for cookie in bernWebCookies(file: cookiesFile, domain: cookieDomain) {
            group.enter()
            store.setCookie(cookie) { group.leave() }
        }
        group.notify(queue: .main) { view.load(URLRequest(url: address)) }
    }
}

#if os(macOS)
extension BernWebPageView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView { makeView(context.coordinator) }
    func updateNSView(_ view: WKWebView, context: Context) { update(view, context.coordinator) }
    static func dismantleNSView(_ view: WKWebView, coordinator: BernWebPageCoordinator) { view.stopLoading() }
}
#else
extension BernWebPageView: UIViewRepresentable {
    func makeUIView(context: Context) -> WKWebView { makeView(context.coordinator) }
    func updateUIView(_ view: WKWebView, context: Context) { update(view, context.coordinator) }
    static func dismantleUIView(_ view: WKWebView, coordinator: BernWebPageCoordinator) { view.stopLoading() }
}
#endif
