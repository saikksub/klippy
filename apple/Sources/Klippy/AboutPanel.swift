import AppKit
import SwiftUI

/// The standard "About Klippy" panel, with company and logo credits (the
/// icon's CC BY 4.0 licence requires attribution).
enum AboutPanel {
    static func show() {
        NSApp.orderFrontStandardAboutPanel(options: [.credits: credits])
        NSApp.activate(ignoringOtherApps: true)
    }

    private static var credits: NSAttributedString {
        let centered = NSMutableParagraphStyle()
        centered.alignment = .center
        let body: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize),
            .foregroundColor: NSColor.labelColor,
            .paragraphStyle: centered,
        ]
        let bold = body.merging([.font: NSFont.boldSystemFont(ofSize: NSFont.smallSystemFontSize)]) { $1 }

        let text = NSMutableAttributedString()
        func add(_ string: String, _ attributes: [NSAttributedString.Key: Any] = body, link: String? = nil) {
            var attributes = attributes
            if let link { attributes[.link] = URL(string: link) }
            text.append(NSAttributedString(string: string, attributes: attributes))
        }

        add("Made by ", body)
        add("Xplorebits Technologies", bold)
        add("\n")
        add("xplorebits.com", link: "https://xplorebits.com")
        add("\n\n")
        add("Logo\n", bold)
        add("solar:paperclip-rounded-2-linear\n")
        add("from the ")
        add("Solar Icons Set", link: "https://www.figma.com/community/file/1166831539721848736/solar-icons-set")
        add(" by 480 Design\n")
        add("Licensed under ")
        add("CC BY 4.0", link: "https://creativecommons.org/licenses/by/4.0/")
        return text
    }
}
