import App
import SwiftUI

extension ExportFormat {
    static let all: [ExportFormat] = [.pdf, .html, .markdown]

    var label: String {
        switch self {
        case .pdf: "PDF"
        case .html: "HTML"
        case .markdown: "Markdown"
        }
    }

    var systemImage: String {
        switch self {
        case .pdf: "doc.richtext"
        case .html: "chevron.left.forwardslash.chevron.right"
        case .markdown: "text.alignleft"
        }
    }
}

/// All three export options side by side, under the viewer card.
struct ExportBar: View {
    let onExport: (ExportFormat) -> Void

    var body: some View {
        HStack(spacing: 8) {
            Label("Export as", systemImage: "square.and.arrow.up")
                .foregroundStyle(.secondary)
            Spacer()
            ForEach(ExportFormat.all, id: \.self) { format in
                Button {
                    onExport(format)
                } label: {
                    Label(format.label, systemImage: format.systemImage)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
                .help("Export as \(format.label)")
            }
        }
    }
}

/// The toolbar's Export menu.
struct ExportMenu: View {
    let onExport: (ExportFormat) -> Void

    var body: some View {
        Menu {
            ForEach(ExportFormat.all, id: \.self) { format in
                Button {
                    onExport(format)
                } label: {
                    Label("\(format.label)…", systemImage: format.systemImage)
                }
            }
        } label: {
            Label("Export", systemImage: "square.and.arrow.up")
        }
        .help("Export")
    }
}
