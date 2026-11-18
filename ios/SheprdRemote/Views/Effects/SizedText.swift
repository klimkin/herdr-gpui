import SheprdRemoteCore
import SwiftUI
import UIKit

/// Text with its `:big[…]` spans drawn at their sizes: each sized run gets
/// a font at its step's multiple of the text style it sits in, heavier for
/// the larger steps, keeping its own bold, italics, and code.
enum SizedText {
    /// `styled` with a font on each run a `TextSizeAttribute` marks, at its
    /// scale of `style` as Dynamic Type sets it at `dynamicType`.
    static func fonts(
        _ styled: AttributedString,
        style: UIFont.TextStyle,
        dynamicType: DynamicTypeSize
    ) -> AttributedString {
        guard styled.runs.contains(where: { $0[TextSizeAttribute.self] != nil }) else { return styled }
        let traits = UITraitCollection(preferredContentSizeCategory: UIContentSizeCategory(dynamicType))
        let base = UIFont.preferredFont(forTextStyle: style, compatibleWith: traits).pointSize
        var out = styled
        for run in styled.runs {
            guard let size = run[TextSizeAttribute.self] else { continue }
            let intent = run.inlinePresentationIntent ?? []
            let heavy = size.heavy || intent.contains(.stronglyEmphasized)
            var font = Font.system(
                size: base * size.scale,
                weight: heavy ? .bold : .regular,
                design: intent.contains(.code) ? .monospaced : .default
            )
            if intent.contains(.emphasized) { font = font.italic() }
            out[run.range].font = font
        }
        return out
    }
}

/// Inline Markdown from the agent, sized.
struct SizedMarkdown: View {
    let text: String
    var style: UIFont.TextStyle = .body
    @Environment(\.dynamicTypeSize) private var dynamicType

    var body: some View {
        Text(SizedText.fonts(InlineMarkdown.attributed(text), style: style, dynamicType: dynamicType))
    }
}

/// What the person wrote, as written but for its size spans.
struct SizedPlain: View {
    let text: String
    var style: UIFont.TextStyle = .body
    @Environment(\.dynamicTypeSize) private var dynamicType

    var body: some View {
        Text(SizedText.fonts(InlineMarkdown.plainSized(text), style: style, dynamicType: dynamicType))
    }
}
