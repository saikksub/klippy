import App
import AppKit
import UniformTypeIdentifiers
import WebKit

/// Saves exports the Rust core prepared: HTML and Markdown as they are, PDF
/// by laying the core's (already sanitised) HTML out into pages.
@MainActor
enum Exporter {
    static func save(_ export: SaveExport) async -> ExportResult {
        guard let url = await destination(for: export) else {
            return ExportResult(path: nil, error: "")
        }
        do {
            switch export.format {
            case .pdf:
                try await PDFRenderer.render(html: String(decoding: export.data, as: UTF8.self), to: url)
            case .html, .markdown:
                try Data(export.data).write(to: url, options: .atomic)
            }
            return ExportResult(path: url.path(percentEncoded: false), error: "")
        } catch {
            return ExportResult(path: nil, error: error.localizedDescription)
        }
    }

    /// A save panel, or a free name in Downloads for quick exports.
    private static func destination(for export: SaveExport) async -> URL? {
        guard export.askLocation else {
            return uniqueURL(in: URL.downloadsDirectory, named: export.fileName)
        }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = export.fileName
        panel.allowedContentTypes = [contentType(export.format)]
        panel.canCreateDirectories = true
        NSApp.activate(ignoringOtherApps: true)
        let response = await withCheckedContinuation { continuation in
            panel.begin { continuation.resume(returning: $0) }
        }
        return response == .OK ? panel.url : nil
    }

    private static func contentType(_ format: ExportFormat) -> UTType {
        switch format {
        case .pdf: .pdf
        case .html: .html
        case .markdown: UTType("net.daringfireball.markdown") ?? .plainText
        }
    }

    /// "Name.pdf", or "Name 2.pdf" etc. if that's taken.
    private static func uniqueURL(in folder: URL, named name: String) -> URL {
        let base = folder.appending(path: name)
        let stem = base.deletingPathExtension().lastPathComponent
        let ext = base.pathExtension
        var url = base
        var number = 2
        while FileManager.default.fileExists(atPath: url.path(percentEncoded: false)) {
            url = folder.appending(path: "\(stem) \(number).\(ext)")
            number += 1
        }
        return url
    }
}

/// Lays HTML out into paginated PDF pages with WebKit, offscreen and without
/// a print dialog, under the same lockdown as the viewer (no JavaScript, no
/// network, nothing stored).
@MainActor
enum PDFRenderer {
    struct Failure: LocalizedError {
        var errorDescription: String? { "The PDF couldn't be created." }
    }

    static func render(html: String, to url: URL) async throws {
        let info = NSPrintInfo()
        info.paperSize = NSPrintInfo.shared.paperSize  // A4 or Letter, by region
        info.topMargin = 48
        info.bottomMargin = 48
        info.leftMargin = 48
        info.rightMargin = 48
        info.horizontalPagination = .fit
        info.verticalPagination = .automatic
        info.jobDisposition = .save
        info.dictionary()[NSPrintInfo.AttributeKey.jobSavingURL] = url

        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.websiteDataStore = .nonPersistent()
        if let rules = await SafeHTMLView.blockEverything() {
            configuration.userContentController.add(rules)
        }
        let width = info.paperSize.width - info.leftMargin - info.rightMargin
        let webView = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: 800), configuration: configuration)
        // Printing needs the view in a window; this one is never shown.
        let window = NSWindow(contentRect: webView.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = webView

        let loader = LoadWaiter()
        webView.navigationDelegate = loader
        try await loader.load(html, in: webView)

        let operation = webView.printOperation(with: info)
        operation.showsPrintPanel = false
        operation.showsProgressPanel = false
        operation.view?.frame = webView.bounds
        let succeeded = await withCheckedContinuation { continuation in
            let done = PrintDone(continuation)
            objc_setAssociatedObject(operation, &PrintDone.key, done, .OBJC_ASSOCIATION_RETAIN)
            operation.runModal(
                for: window,
                delegate: done,
                didRun: #selector(PrintDone.printOperationDidRun(_:success:contextInfo:)),
                contextInfo: nil
            )
        }
        guard succeeded else { throw Failure() }
    }

    /// Loads the page, allowing nothing but that first load.
    private final class LoadWaiter: NSObject, WKNavigationDelegate {
        private var continuation: CheckedContinuation<Void, Error>?

        func load(_ html: String, in webView: WKWebView) async throws {
            try await withCheckedThrowingContinuation { continuation in
                self.continuation = continuation
                webView.loadHTMLString(html, baseURL: nil)
            }
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor action: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            decisionHandler(action.request.url?.absoluteString == "about:blank" ? .allow : .cancel)
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            continuation?.resume()
            continuation = nil
        }

        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            continuation?.resume(throwing: error)
            continuation = nil
        }
    }

    private final class PrintDone: NSObject {
        nonisolated(unsafe) static var key = 0
        private let continuation: CheckedContinuation<Bool, Never>

        init(_ continuation: CheckedContinuation<Bool, Never>) {
            self.continuation = continuation
        }

        @objc func printOperationDidRun(
            _ operation: NSPrintOperation,
            success: Bool,
            contextInfo: UnsafeMutableRawPointer?
        ) {
            continuation.resume(returning: success)
        }
    }
}
