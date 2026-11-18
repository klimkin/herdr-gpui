import Foundation
import Testing
@testable import SheprdRemoteCore

/// Mirrors `chat/rich/tests/sizes.rs`.
@Suite struct TextSizeTests {
    /// The text, each run with the size it reads at.
    private func runs(_ source: String) -> [(String, TextSize?)] {
        let styled = InlineMarkdown.attributed(source)
        var runs: [(String, TextSize?)] = []
        for run in styled.runs {
            let text = String(styled[run.range].characters)
            if let last = runs.last, last.1 == run[TextSizeAttribute.self] {
                runs[runs.count - 1].0 += text
            } else {
                runs.append((text, run[TextSizeAttribute.self]))
            }
        }
        return runs
    }

    private func text(_ runs: [(String, TextSize?)]) -> String {
        runs.map(\.0).joined()
    }

    @Test func aSizeSpanSetsItsTextAndLeavesOutItsMarks() {
        let runs = runs("Tests pass and :huge[the PR is merged]. :small[CI took 4 min.]")
        #expect(text(runs) == "Tests pass and the PR is merged. CI took 4 min.")
        #expect(runs.contains { $0 == "the PR is merged" && $1 == .huge })
        #expect(runs.contains { $0 == "CI took 4 min." && $1 == .small })
        #expect(runs.contains { $0 == "Tests pass and " && $1 == nil })
    }

    @Test func aSpanKeepsInlineStylesAndItsOwnBrackets() {
        let source = ":big[a **bold** [note] word] after"
        let styled = InlineMarkdown.attributed(source)
        #expect(String(styled.characters) == "a bold [note] word after")
        let bold = styled.runs.first { String(styled[$0.range].characters) == "bold" }
        #expect(bold?[TextSizeAttribute.self] == .big)
        #expect(bold?.inlinePresentationIntent == .stronglyEmphasized)
        #expect(runs(source).contains { $0.contains("[note] word") && $1 == .big })
        #expect(runs(source).last.map { $0 == " after" && $1 == nil } == true)
    }

    @Test func aLinkInsideASpanStaysALink() {
        let styled = InlineMarkdown.attributed(":huge[see [the PR](https://example.com/1)]")
        let link = styled.runs.first { $0.link != nil }
        #expect(link?.link?.absoluteString == "https://example.com/1")
        #expect(link?[TextSizeAttribute.self] == .huge)
    }

    @Test func unknownNamesCodeAndAddressesStayAsWritten() {
        for source in [":large[x]", "`:big[x]`", "see http://host:big[x]", "word:big[x]", "\\:big[x]"] {
            let runs = runs(source)
            #expect(runs.allSatisfy { $0.1 == nil }, "\(source)")
        }
        #expect(String(InlineMarkdown.attributed("`:big[x]`").characters) == ":big[x]")
        #expect(String(InlineMarkdown.attributed("word:big[x]").characters) == "word:big[x]")
    }

    @Test func anUnclosedSpanEndsWithItsParagraph() {
        let blocks = MarkdownBlock.parse(":giant[Still streaming\n\nNext")
        let sizes = blocks.map { block -> TextSize? in
            guard case let .paragraph(text) = block else { return nil }
            return InlineMarkdown.attributed(text).runs.first?[TextSizeAttribute.self]
        }
        #expect(sizes == [.giant, nil])
    }

    @Test func spansNestToABound() {
        let deep = String(repeating: ":big[", count: 20) + "x" + String(repeating: "]", count: 20)
        let runs = runs(deep)
        // The deepest openings beyond the bound stay text, but the text shows.
        #expect(text(runs).contains("x"))
        #expect(runs.contains { $0.1 == .big })
        #expect(InlineMarkdown.mark(deep).filter { $0 == InlineMarkdown.closing }.count == InlineMarkdown.maxOpen)
    }

    @Test func theInnermostSpanSetsTheSize() {
        let runs = runs(":big[a :small[b] c]")
        #expect(runs.map(\.0) == ["a ", "b", " c"])
        #expect(runs.map(\.1) == [.big, .small, .big])
    }

    @Test func marksTheAgentWroteItselfAreDropped() {
        #expect(String(InlineMarkdown.attributed("a\u{E002}b\u{E000}c").characters) == "abc")
        #expect(runs("a\u{E002}b").allSatisfy { $0.1 == nil })
    }

    @Test func theUsersOwnTextTakesOnlyItsSizes() {
        let styled = InlineMarkdown.plainSized("Ship it **now** :big[today]")
        #expect(String(styled.characters) == "Ship it **now** today")
        #expect(styled.runs.last?[TextSizeAttribute.self] == .big)
    }

    @Test func onlyWebLinksStayLinks() {
        let styled = InlineMarkdown.attributed("[a](https://x.dev) [b](herdr-remote://chat?id=1)")
        #expect(styled.runs.compactMap(\.link).map(\.absoluteString) == ["https://x.dev"])
    }

    @Test func largerStepsReadHeavier() {
        #expect(TextSize.small.scale < 1 && !TextSize.small.heavy)
        #expect([TextSize.big, .huge, .giant].map(\.scale) == [1.35, 2, 3])
        #expect([TextSize.big, .huge, .giant].allSatisfy { $0.heavy })
    }

    @Test func plainTextDropsEveryMark() {
        #expect(InlineMarkdown.plain("Tests :big[pass]: **214**") == "Tests pass: 214")
    }
}
