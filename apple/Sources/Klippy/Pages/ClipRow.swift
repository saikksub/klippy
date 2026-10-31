import App
import SwiftUI

/// A rounded card for one history item. Hovering shows a Copy button and,
/// when given, Export and Open buttons.
struct ClipRow: View {
    let item: ClipItem
    var isSelected = false
    let onTap: () -> Void
    let onCopy: () -> Void
    var onExport: (() -> Void)?
    var exportHelp = "Export"
    var onOpen: (() -> Void)?

    @State private var isHovered = false
    @State private var justCopied = false

    var body: some View {
        HStack(spacing: 6) {
            Text(item.preview)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)

            // Always laid out so the text doesn't reflow on hover.
            HStack(spacing: 4) {
                RowIconButton(systemImage: justCopied ? "checkmark" : "doc.on.doc", help: "Copy", action: copy)
                if let onExport {
                    RowIconButton(systemImage: "square.and.arrow.down", help: exportHelp, action: onExport)
                }
                if let onOpen {
                    RowIconButton(systemImage: "chevron.right", help: "Open in History", action: onOpen)
                }
            }
            .opacity(isHovered || justCopied ? 1 : 0)
            .allowsHitTesting(isHovered)
        }
        .padding(.leading, 14)
        .padding(.trailing, 8)
        .frame(height: 44)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(background)
        )
        .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onTapGesture(perform: onTap)
        .onHover { isHovered = $0 }
        .animation(.easeOut(duration: 0.12), value: isHovered)
    }

    private var background: Color {
        if isSelected { return Color.accentColor.opacity(0.35) }
        return Color.primary.opacity(isHovered ? 0.1 : 0.06)
    }

    private func copy() {
        onCopy()
        justCopied = true
        Task {
            try? await Task.sleep(for: .seconds(1))
            justCopied = false
        }
    }
}

private struct RowIconButton: View {
    let systemImage: String
    let help: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 11, weight: .semibold))
                .frame(width: 26, height: 26)
                .background(Circle().fill(Color.primary.opacity(0.1)))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(help)
    }
}
