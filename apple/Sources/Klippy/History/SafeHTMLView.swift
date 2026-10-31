import SwiftUI
import WebKit

/// Renders untrusted copied HTML with nothing able to run or reach out:
/// - JavaScript is off.
/// - A content rule blocks every subresource (images, CSS, fonts, frames,
///   fetches), and a Content-Security-Policy repeats that inside the page.
/// - Every navigation after the initial load is cancelled, so links,
///   redirects and frames go nowhere.
/// - Nothing is stored: the data store is in-memory only.
/// WebKit also renders in a separate, OS-sandboxed process.
struct SafeHTMLView: NSViewRepresentable {
    let html: String

    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.websiteDataStore = .nonPersistent()
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = context.coordinator
        webView.allowsLinkPreview = false
        webView.allowsMagnification = true
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        guard context.coordinator.shownHTML != html else { return }
        context.coordinator.shownHTML = html
        let page = Self.contentSecurityPolicy + html
        Task { @MainActor in
            // Rules must be in place before loading, or images would load first.
            guard let rules = await Self.blockEverything() else { return }
            webView.configuration.userContentController.removeAllContentRuleLists()
            webView.configuration.userContentController.add(rules)
            webView.loadHTMLString(page, baseURL: nil)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var shownHTML: String?

        /// Only the page itself, loaded as about:blank, is allowed.
        func webView(
            _ webView: WKWebView,
            decidePolicyFor action: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            let isInitialLoad = action.navigationType == .other
                && action.targetFrame?.isMainFrame == true
                && action.request.url?.absoluteString == "about:blank"
            decisionHandler(isInitialLoad ? .allow : .cancel)
        }
    }

    /// Inline styles only; no scripts, no network, images only from data: URIs.
    private static let contentSecurityPolicy = """
    <meta http-equiv="Content-Security-Policy" \
    content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">
    """

    private static let ruleList = """
    [{"trigger": {"url-filter": ".*", "resource-type": ["image", "style-sheet", "script", "font", \
    "raw", "svg-document", "media", "popup", "ping", "fetch", "websocket", "other"]}, \
    "action": {"type": "block"}}]
    """

    @MainActor private static var compiled: WKContentRuleList?

    @MainActor private static func blockEverything() async -> WKContentRuleList? {
        if let compiled { return compiled }
        compiled = try? await WKContentRuleListStore.default()
            .compileContentRuleList(forIdentifier: "klippy-block-everything", encodedContentRuleList: ruleList)
        return compiled
    }
}
