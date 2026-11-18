import Foundation

/// Text an agent or the person sets larger or smaller within a line, as
/// `:big[text]` asks (`chat/rich/size.rs` on the desktop). The writer names
/// one of a few steps, never a size of its own, so one word cannot fill the
/// screen. A name that is not a step stays text.
public enum TextSize: String, CaseIterable, Codable, Hashable, Sendable {
    // keep-sorted start
    case big
    case giant
    case huge
    case small
    // keep-sorted end

    /// The span's text size, as a multiple of the prose's.
    public var scale: Double {
        switch self {
        case .small: 0.85
        case .big: 1.35
        case .huge: 2
        case .giant: 3
        }
    }

    /// Whether the span reads heavier than the prose, as larger text does.
    public var heavy: Bool { self != .small }

    /// The private-use character that stands for the span's opening while
    /// Markdown parses the text around it.
    fileprivate var mark: Character {
        switch self {
        case .small: "\u{E001}"
        case .big: "\u{E002}"
        case .huge: "\u{E003}"
        case .giant: "\u{E004}"
        }
    }

    fileprivate init?(mark: Character) {
        guard let size = Self.allCases.first(where: { $0.mark == mark }) else { return nil }
        self = size
    }
}

/// The size a run of inline text reads at, on an `AttributedString`.
public enum TextSizeAttribute: CodableAttributedStringKey {
    public typealias Value = TextSize
    public static let name = "sheprd.textSize"
}

/// Inline Markdown as the chat shows it: `:big[…]` spans set their text
/// larger or smaller, and only web links stay links.
public enum InlineMarkdown {
    /// Spans open at once; deeper openings stay text.
    static let maxOpen = 8
    /// What stands for a span's closing `]` while Markdown parses.
    static let closing: Character = "\u{E000}"
    static let schemes: Set<String> = ["http", "https"]

    /// `text` as inline Markdown, with each sized run marked by a
    /// `TextSizeAttribute`. Agent text is untrusted: only `http` and `https`
    /// links stay links, so transcript text can never follow the app's own
    /// scheme.
    public static func attributed(_ text: String) -> AttributedString {
        let marked = mark(text)
        let parsed = (try? AttributedString(
            markdown: marked,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        )) ?? AttributedString(marked)
        var styled = sized(parsed)
        for run in styled.runs {
            if let link = run.link, !schemes.contains(link.scheme?.lowercased() ?? "") {
                styled[run.range].link = nil
            }
        }
        return styled
    }

    /// `text` as written, with only its size spans taken in: what the
    /// person's own message shows, since it is not Markdown on the phone.
    public static func plainSized(_ text: String) -> AttributedString {
        sized(AttributedString(mark(text)))
    }

    /// The words `text` shows, without Markdown's marks or the spans'.
    public static func plain(_ text: String) -> String {
        String(attributed(text).characters)
    }

    /// `text` with each span's opening `:big[` and its closing `]` replaced
    /// by private-use marks Markdown leaves alone, so bold, italics, and
    /// links inside a span still parse. Code spans and escapes stay as
    /// written, and an opening after a letter or `/`, as in `word:big[` or
    /// `http://x:big[`, is text.
    static func mark(_ text: String) -> String {
        // Marks the agent wrote itself would pass for spans: drop them.
        let source = Array(text.filter { $0 != closing && TextSize(mark: $0) == nil })
        var out = ""
        out.reserveCapacity(source.count)
        // Each open span with how many of its own brackets are open in it.
        var open: [Int] = []
        var index = 0
        while index < source.count {
            let character = source[index]
            switch character {
            case "\\" where index + 1 < source.count:
                out.append(character)
                out.append(source[index + 1])
                index += 2
                continue
            case "`":
                let run = source[index...].prefix { $0 == "`" }.count
                if let end = closingTicks(source, from: index + run, run: run) {
                    out.append(contentsOf: source[index..<end])
                    index = end
                } else {
                    out.append(contentsOf: source[index..<index + run])
                    index += run
                }
                continue
            case ":" where startsDirective(before: index > 0 ? source[index - 1] : nil):
                if open.count < maxOpen, let (size, length) = opening(source, at: index + 1) {
                    out.append(size.mark)
                    open.append(1)
                    index += 1 + length
                    continue
                }
            case "[":
                if !open.isEmpty { open[open.count - 1] += 1 }
            case "]":
                if let depth = open.last {
                    if depth == 1 {
                        out.append(closing)
                        open.removeLast()
                        index += 1
                        continue
                    }
                    open[open.count - 1] -= 1
                }
            default:
                break
            }
            out.append(character)
            index += 1
        }
        return out
    }

    /// Whether a directive may begin after `before`: at the start, or after
    /// a character that is not part of a word or an address.
    static func startsDirective(before: Character?) -> Bool {
        guard let before else { return true }
        return !(before.isLetter || before.isNumber) && before != "/"
    }

    /// The step `source` names at `start`, as `big[` does, with the
    /// characters it takes.
    private static func opening(_ source: [Character], at start: Int) -> (TextSize, Int)? {
        for size in TextSize.allCases {
            let name = Array(size.rawValue)
            let end = start + name.count
            guard end < source.count, Array(source[start..<end]) == name, source[end] == "[" else { continue }
            return (size, name.count + 1)
        }
        return nil
    }

    /// Where the code span that opened with `run` backticks ends, just
    /// after its closing run of the same length.
    private static func closingTicks(_ source: [Character], from start: Int, run: Int) -> Int? {
        var index = start
        while index < source.count {
            guard source[index] == "`" else {
                index += 1
                continue
            }
            let length = source[index...].prefix { $0 == "`" }.count
            if length == run { return index + length }
            index += length
        }
        return nil
    }

    /// `marked` with its marks taken out and each run between them carrying
    /// the size of the innermost span open there. A span still open at the
    /// end ends with the text, as a paragraph ending ends it.
    static func sized(_ marked: AttributedString) -> AttributedString {
        var out = AttributedString()
        var open: [TextSize] = []
        var start = marked.startIndex
        func take(upTo end: AttributedString.Index) {
            guard start < end else { return }
            var piece = AttributedString(marked[start..<end])
            if let size = open.last { piece[TextSizeAttribute.self] = size }
            out.append(piece)
        }
        var index = marked.startIndex
        while index < marked.endIndex {
            let character = marked.characters[index]
            let next = marked.characters.index(after: index)
            if character == closing {
                take(upTo: index)
                if !open.isEmpty { open.removeLast() }
                start = next
            } else if let size = TextSize(mark: character) {
                take(upTo: index)
                open.append(size)
                start = next
            }
            index = next
        }
        take(upTo: marked.endIndex)
        return out
    }
}
