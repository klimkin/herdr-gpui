import Foundation

/// An effect on the Markdown an agent wraps between `:::slam` and `:::`
/// (`chat/effects.rs` on the desktop).
public enum BubbleEffect: String, CaseIterable, Codable, Hashable, Sendable {
    // keep-sorted start
    case gentle
    case ink
    case loud
    case slam
    // keep-sorted end

    /// The name its replay link shows.
    public var label: String {
        switch self {
        case .gentle: "Gentle"
        case .ink: "Invisible ink"
        case .loud: "Loud"
        case .slam: "Slam"
        }
    }

    /// How long it plays, in seconds.
    public var length: Double {
        switch self {
        case .gentle: 2.2
        case .ink: 4
        case .loud: 1.6
        case .slam: 0.9
        }
    }

    /// Longer than any bubble plays, after which its start is forgotten.
    public static let longest = allCases.map(\.length).max() ?? 0

    /// The effect a line such as `:::slam` opens.
    public init?(opening line: String) {
        guard let trimmed = directive(line), trimmed.hasPrefix(":::"), let effect = Self(rawValue: String(trimmed.dropFirst(3))) else { return nil }
        self = effect
    }

    /// Whether a line is the `:::` that ends a bubble.
    public static func closes(_ line: String) -> Bool {
        line.trimmingCharacters(in: .whitespaces) == ":::"
    }
}

/// An effect over the chat screen, and nothing beyond it, that a line such
/// as `::confetti` asks for.
public enum ScreenEffect: String, CaseIterable, Codable, Hashable, Sendable {
    // keep-sorted start
    case balloons
    case celebration
    case confetti
    case echo
    case fireworks
    case lasers
    case love
    case spotlight
    case zap
    // keep-sorted end

    public var label: String { rawValue.prefix(1).uppercased() + rawValue.dropFirst() }

    /// How long it plays, in seconds.
    public var length: Double {
        switch self {
        case .balloons: 4.2
        case .celebration: 3.4
        case .confetti: 3.4
        case .echo: 3.8
        case .fireworks: 4
        case .lasers: 3.6
        case .love: 3
        case .spotlight: 3.3
        case .zap: 4.6
        }
    }

    /// The effect a line such as `::confetti` asks for. `:::slam` is a
    /// bubble's opening, not a screen effect named `:slam`.
    public init?(line: String) {
        guard let trimmed = directive(line), trimmed.hasPrefix("::"), !trimmed.hasPrefix(":::"),
              let effect = Self(rawValue: String(trimmed.dropFirst(2)))
        else { return nil }
        self = effect
    }
}

/// A line trimmed, if it may hold a directive: indented by three spaces at
/// most, as a line indented further is code.
private func directive(_ line: String) -> String? {
    let indent = line.prefix { $0 == " " || $0 == "\t" }
    guard indent.count <= 3, !indent.contains("\t") else { return nil }
    return line.trimmingCharacters(in: .whitespaces)
}

/// An agent's message split at its effects: Markdown, the Markdown its
/// bubbles wrap, and the screen effect it asks for. No directive is left in
/// the Markdown, so none shows as written.
public struct AgentMessage: Hashable, Sendable {
    public enum Part: Hashable, Sendable {
        case markdown([MarkdownBlock])
        case bubble(BubbleEffect, [MarkdownBlock])
    }

    public var parts: [Part]
    /// The first screen effect the message asks for; later ones are dropped.
    public var screen: ScreenEffect?

    /// Bubbles per message; later ones show their Markdown without effect.
    public static let maxBubbles = 8

    public init(parts: [Part], screen: ScreenEffect? = nil) {
        self.parts = parts
        self.screen = screen
    }

    /// How many bubbles play in the message.
    public var bubbles: Int {
        parts.count { if case .bubble = $0 { true } else { false } }
    }

    /// The message's words in brief, for the effects that draw them: its
    /// first paragraph's, without Markdown, at most `limit` characters.
    public func gist(limit: Int = 48) -> String {
        let paragraph = parts.lazy.flatMap { part -> [MarkdownBlock] in
            switch part {
            case let .markdown(blocks), let .bubble(_, blocks): blocks
            }
        }
        .compactMap { block -> String? in if case let .paragraph(text) = block { text } else { nil } }
        .first
        guard let paragraph else { return "" }
        let words = InlineMarkdown.plain(paragraph)
            .map { $0.isNewline ? " " : $0 }
            .filter { !$0.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) } }
        return String(String(words).trimmingCharacters(in: .whitespaces).prefix(limit))
    }

    /// A reply flattened to one line, as a list row shows the computer's
    /// excerpt of it, without the directives the chat plays: effect names
    /// and `:::` are dropped, and size spans keep only their words.
    public static func rowLine(_ line: String) -> String {
        let words = line.split(separator: " ", omittingEmptySubsequences: true).filter { word in
            word != ":::" && BubbleEffect(opening: String(word)) == nil && ScreenEffect(line: String(word)) == nil
        }
        return String(InlineMarkdown.plainSized(words.joined(separator: " ")).characters)
    }

    /// Splits an agent's `text` at its effects. A `:::` inside a code fence
    /// does not close a bubble, and a directive inside one is code; a
    /// bubble still streaming reaches the end of the message.
    public static func parse(_ text: String) -> AgentMessage {
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map {
            $0.hasSuffix("\r") ? String($0.dropLast()) : String($0)
        }
        var message = AgentMessage(parts: [])
        var markdown: [String] = []
        var fence: OpenFence?
        var bubbles = 0
        var index = 0
        func flush() {
            let blocks = MarkdownBlock.parse(markdown.joined(separator: "\n"))
            if !blocks.isEmpty { message.parts.append(.markdown(blocks)) }
            markdown.removeAll()
        }
        while index < lines.count {
            let line = lines[index]
            index += 1
            if let open = fence {
                if open.closes(line) { fence = nil }
                markdown.append(line)
            } else if let open = OpenFence(opening: line) {
                fence = open
                markdown.append(line)
            } else if let effect = ScreenEffect(line: line) {
                message.screen = message.screen ?? effect
            } else if let effect = BubbleEffect(opening: line) {
                flush()
                var inner: [String] = []
                var code: OpenFence?
                while index < lines.count {
                    let line = lines[index]
                    index += 1
                    if let open = code {
                        if open.closes(line) { code = nil }
                    } else if BubbleEffect.closes(line) {
                        break
                    } else if let screen = ScreenEffect(line: line) {
                        message.screen = message.screen ?? screen
                        continue
                    } else {
                        code = OpenFence(opening: line)
                    }
                    inner.append(line)
                }
                let blocks = MarkdownBlock.parse(inner.joined(separator: "\n"))
                guard !blocks.isEmpty else { continue }
                if bubbles < maxBubbles {
                    bubbles += 1
                    message.parts.append(.bubble(effect, blocks))
                } else {
                    message.parts.append(.markdown(blocks))
                }
            } else {
                markdown.append(line)
            }
        }
        flush()
        return message
    }
}
