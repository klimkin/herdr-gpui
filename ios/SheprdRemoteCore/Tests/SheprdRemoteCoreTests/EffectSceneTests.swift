import CoreGraphics
import Testing
@testable import SheprdRemoteCore

/// Mirrors `chat/effects/tests.rs`: every frame is a function of time alone.
@Suite struct EffectSceneTests {
    private let screen = CGSize(width: 480, height: 640)

    private func marks(_ effect: ScreenEffect, _ t: Double, size: CGSize? = nil, letters: Int = 20) -> [EffectScene.Mark] {
        EffectScene.marks(effect, at: t, size: size ?? screen, letters: letters)
    }

    private func letters(_ marks: [EffectScene.Mark]) -> [(x: Double, y: Double, size: Double)] {
        marks.compactMap { mark in
            if case let .words(x, y, size, _, .letter) = mark { (x, y, size) } else { nil }
        }
    }

    @Test func everyScreenEffectDrawsWhileItPlaysAndNothingAfter() {
        for effect in ScreenEffect.allCases {
            #expect(!marks(effect, effect.length * 0.4).isEmpty, "\(effect)")
            #expect(marks(effect, effect.length).isEmpty, "\(effect)")
            #expect(marks(effect, -0.1).isEmpty, "\(effect)")
        }
    }

    @Test func aFrameDependsOnTheTimeAlone() {
        for effect in ScreenEffect.allCases {
            #expect(marks(effect, 1.3) == marks(effect, 1.3), "\(effect)")
        }
    }

    @Test func everyMarkIsVisibleOrFaintNeverBeyondOpaque() {
        for effect in ScreenEffect.allCases {
            for step in 0..<40 {
                for mark in marks(effect, effect.length * Double(step) / 40) {
                    let alpha = switch mark {
                    case let .box(_, _, _, alpha), let .beam(_, _, _, _, alpha), let .words(_, _, _, alpha, _),
                         let .veil(alpha, _): alpha
                    }
                    #expect((0...1).contains(alpha), "\(effect) \(mark)")
                }
            }
        }
    }

    @Test func aSpotlightSettlesOnItsMessage() {
        let target = CGRect(x: 40, y: 300, width: 200, height: 40)
        let marks = EffectScene.marks(.spotlight, at: 2, size: screen, target: target)
        guard marks.count == 1, case let .veil(_, hole?) = marks[0] else {
            Issue.record("\(marks)")
            return
        }
        #expect(hole.contains(target))
    }

    @Test func anEmptyScreenDrawsNothing() {
        #expect(marks(.confetti, 1, size: CGSize(width: 0, height: 640)).isEmpty)
    }

    @Test func aBubbleRestsWhenItNeverPlayedOrIsOver() {
        for effect in BubbleEffect.allCases {
            #expect(effect.pose(at: nil) == .rest)
            #expect(effect.pose(at: effect.length) == .rest)
        }
    }

    @Test func aSlamLandsFromLargeAndLoudGrowsThenReturns() {
        let slam = BubbleEffect.slam.pose(at: 0)
        #expect(slam.scale > 2 && slam.opacity < 0.1)
        #expect(BubbleEffect.loud.pose(at: 0.6).scale > 1.4)
        let gentle = BubbleEffect.gentle.pose(at: 0.2)
        #expect(gentle.scale < 0.7 && gentle.opacity < 0.5)
        #expect(BubbleEffect.ink.pose(at: 1).shimmer)
    }

    @Test func specksStayInsideTheTextTheyHide() {
        for frame: UInt32 in [0, 1, 99] {
            for index: UInt32 in 0..<500 {
                let speck = EffectScene.speck(index, frame: frame)
                #expect((0..<1).contains(speck.x) && (0..<1).contains(speck.y))
                #expect((0...1).contains(speck.alpha))
            }
        }
        #expect(EffectScene.speck(3, frame: 1) != EffectScene.speck(3, frame: 2), "they shimmer from frame to frame")
    }

    @Test func theHashMatchesTheDesktops() {
        // The same scatter as `scene::unit`, so an effect plays alike on both.
        #expect(EffectScene.unit(0, 0) == 0)
        #expect(EffectScene.unit(1, 2) == 0.05626040697097778)
        #expect(EffectScene.unit(7, 41) == 0.8903857469558716)
    }

    @Test func zapShootsDownEveryLetterBeforeItEnds() {
        #expect(letters(marks(.zap, 0.1)).count == 20, "the whole formation shows at first")
        #expect(letters(marks(.zap, ScreenEffect.zap.length - 0.6)).isEmpty, "every letter is hit by the end")
        #expect(letters(marks(.zap, 2)).count < 20, "some are hit along the way")
    }

    @Test func zapDrawsAtMostItsFormation() {
        #expect(letters(marks(.zap, 0.1, letters: 500)).count == Zap.maxTargets)
    }

    @Test func zapKeepsEveryLetterInsideANarrowOrShortScreen() {
        for (width, height) in [(120.0, 300.0), (120, 80), (200, 160), (300, 640), (480, 120), (1200, 640)] {
            for step in 0..<46 {
                let marks = marks(.zap, Double(step) * 0.1, size: CGSize(width: width, height: height), letters: 24)
                for letter in letters(marks) {
                    // A letter is about half as wide as it is tall; `y` is its top.
                    #expect(letter.x - letter.size / 2 >= 0 && letter.x + letter.size / 2 <= width, "\(width)x\(height)")
                    #expect(letter.y >= 0 && letter.y + letter.size <= height, "\(width)x\(height)")
                }
            }
        }
    }

    @Test func zapsLettersAreTheMessagesOwn() {
        #expect(EffectScene.letters(in: "Ship it!") == 7)
        #expect(EffectScene.letter(of: "Ship it!", at: 4) == "i")
        #expect(EffectScene.letter(of: "ab", at: 5) == "")
    }
}
