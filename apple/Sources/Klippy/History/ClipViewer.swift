import App
import AppKit
import SwiftUI

/// Shows a copied item as it was copied, without letting it run anything:
/// RTF/RTFD with attachments and links stripped, HTML in a locked-down web
/// view, otherwise plain text.
struct ClipViewer: View {
    let clip: SelectedClip

    var body: some View {
        if let rich = Self.richText(clip) {
            SafeTextView(text: rich, id: clip.id, isRich: true)
        } else if let html = Self.format(.html, of: clip) {
            SafeHTMLView(html: String(decoding: html, as: UTF8.self))
        } else {
            SafeTextView(
                text: NSAttributedString(string: clip.text, attributes: [
                    .font: NSFont.monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular),
                    .foregroundColor: NSColor.labelColor,
                ]),
                id: clip.id,
                isRich: false
            )
        }
    }

    private static func format(_ kind: NSPasteboard.PasteboardType, of clip: SelectedClip) -> Data? {
        clip.formats.first { $0.kind == kind.rawValue }.map { Data($0.data) }
    }

    /// RTF never loads anything remotely, but RTFD can embed files and both
    /// can carry links, so those are removed before display.
    private static func richText(_ clip: SelectedClip) -> NSAttributedString? {
        let parsed = format(.rtfd, of: clip).flatMap { NSAttributedString(rtfd: $0, documentAttributes: nil) }
            ?? format(.rtf, of: clip).flatMap { NSAttributedString(rtf: $0, documentAttributes: nil) }
        return parsed.map(sanitized)
    }

    private static func sanitized(_ text: NSAttributedString) -> NSAttributedString {
        let clean = NSMutableAttributedString(attributedString: text)
        let whole = NSRange(location: 0, length: clean.length)
        clean.removeAttribute(.link, range: whole)
        // Back to front so earlier ranges stay valid while deleting.
        var attachments: [NSRange] = []
        clean.enumerateAttribute(.attachment, in: whole) { value, range, _ in
            if value != nil { attachments.append(range) }
        }
        for range in attachments.reversed() {
            clean.replaceCharacters(in: range, with: "[attachment removed]")
        }
        return clean
    }
}

/// Read-only, selectable text with no link detection or attachment handling.
private struct SafeTextView: NSViewRepresentable {
    let text: NSAttributedString
    let id: UInt64
    let isRich: Bool

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let textView = scrollView.documentView as! NSTextView
        textView.isEditable = false
        textView.isSelectable = true
        textView.importsGraphics = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.textContainerInset = NSSize(width: 16, height: 16)
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard context.coordinator.shownID != id else { return }
        context.coordinator.shownID = id
        let textView = scrollView.documentView as! NSTextView
        // Rich text carries its own (usually dark) colours, so show it on a
        // light page as it looked where it was copied from.
        textView.backgroundColor = isRich ? .white : .textBackgroundColor
        textView.textStorage?.setAttributedString(text)
        textView.scrollToBeginningOfDocument(nil)
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator {
        var shownID: UInt64?
    }
}
