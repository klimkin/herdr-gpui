import Testing
@testable import SheprdRemoteCore

/// Mirrors the directive tests in `chat/tests/effects.rs`.
@Suite struct AgentMessageTests {
    /// The message's parts, in brief, then its screen effect.
    private func shape(_ text: String) -> [String] {
        let message = AgentMessage.parse(text)
        let parts = message.parts.map { part -> String in
            switch part {
            case let .markdown(blocks): "text:" + blocks.map(Self.words).joined(separator: " ")
            case let .bubble(effect, _): "bubble:\(effect.rawValue)"
            }
        }
        return parts + (message.screen.map { ["screen:\($0.rawValue)"] } ?? [])
    }

    private static func words(_ block: MarkdownBlock) -> String {
        switch block {
        case let .paragraph(text): text.replacingOccurrences(of: "\n", with: " ")
        case let .code(_, text): "code:" + text
        default: "\(block)"
        }
    }

    @Test func aBubbleEffectWrapsTheMarkdownBetweenItsFences() {
        #expect(shape("Before\n:::slam\nFound **it**.\n:::\nAfter") == ["text:Before", "bubble:slam", "text:After"])
        let message = AgentMessage.parse(":::slam\nFound **it**.\n:::")
        #expect(message.parts == [.bubble(.slam, [.paragraph("Found **it**.")])])
    }

    @Test func aBubbleStillStreamingReachesTheEndOfTheMessage() {
        #expect(shape("Hi\n:::ink\nThe key is") == ["text:Hi", "bubble:ink"])
        // Only its opening so far: nothing to play yet.
        #expect(shape("Hi\n:::loud\n") == ["text:Hi"])
    }

    @Test func aScreenEffectLineIsTakenOutAndOnlyTheFirstCounts() {
        #expect(shape("::confetti\nAll tests pass.\n::fireworks") == ["text:All tests pass.", "screen:confetti"])
    }

    @Test func unknownOrIndentedOrFencedDirectivesStayText() {
        #expect(shape("::sparkles\nHi") == ["text:::sparkles Hi"])
        #expect(shape(":::wobble\nHi\n:::") == ["text::::wobble Hi :::"])
        #expect(!shape("```\n::confetti\n```").contains { $0.hasPrefix("screen") })
        #expect(!shape("    ::confetti").contains { $0.hasPrefix("screen") })
        #expect(!shape("\t:::slam\nx\n:::").contains { $0.hasPrefix("bubble") })
        #expect(shape("   ::love\nok") == ["text:ok", "screen:love"])
    }

    @Test func aFencedClosingLineStaysInTheBubble() {
        #expect(shape(":::slam\n```markdown\n:::\n```\nafter\n:::\nOutside") == ["bubble:slam", "text:Outside"])
        guard case let .bubble(_, blocks) = AgentMessage.parse(":::slam\n```\n:::\n```\n:::").parts.first else {
            Issue.record("bubble")
            return
        }
        #expect(blocks == [.code(language: nil, text: ":::")])
    }

    @Test func aScreenLineInsideABubbleIsTakenOutToo() {
        #expect(shape(":::gentle\nWell done\n::love\n:::") == ["bubble:gentle", "screen:love"])
    }

    @Test func bubblesBeyondTheBoundShowTheirWordsWithoutEffect() {
        let text = (0..<10).map { ":::slam\nline \($0)\n:::" }.joined(separator: "\n")
        let message = AgentMessage.parse(text)
        #expect(message.bubbles == AgentMessage.maxBubbles)
        #expect(message.parts.count == 10)
        #expect(message.parts.last == .markdown([.paragraph("line 9")]))
    }

    @Test func everyEffectHasADirectiveNameAndLabel() {
        for effect in BubbleEffect.allCases {
            #expect(BubbleEffect(opening: ":::\(effect.rawValue)") == effect)
            #expect(!effect.label.isEmpty)
        }
        for effect in ScreenEffect.allCases {
            #expect(ScreenEffect(line: "::\(effect.rawValue)") == effect)
            #expect(ScreenEffect(line: ":::\(effect.rawValue)") == nil)
            #expect(!effect.label.isEmpty)
        }
        #expect(ScreenEffect.confetti.label == "Confetti")
        #expect(BubbleEffect.ink.label == "Invisible ink")
    }

    @Test func theGistIsTheFirstParagraphsWords() {
        #expect(AgentMessage.parse("::zap\n# Title\n\nAll **214** pass :big[now]\nsecond line").gist()
            == "All 214 pass now second line")
        #expect(AgentMessage.parse("::echo\n").gist() == "")
        #expect(AgentMessage.parse(String(repeating: "a", count: 100)).gist().count == 48)
    }

    @Test func aRowLineDropsTheDirectivesItsExcerptFlattened() {
        #expect(AgentMessage.rowLine("::confetti :::slam Tests :big[pass] ::: done") == "Tests pass done")
        #expect(AgentMessage.rowLine(":huge[214 of 2…") == "214 of 2…")
        #expect(AgentMessage.rowLine("::sparkles stays") == "::sparkles stays")
    }
}
