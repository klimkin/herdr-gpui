import Foundation
import Testing
@testable import SheprdRemoteCore

/// Mirrors the play-once rules in `chat/tests/effects.rs`.
@Suite struct ChatEffectsTests {
    private let now = Date(timeIntervalSinceReferenceDate: 1000)

    private func transcript(_ texts: [String]) -> Transcript {
        var transcript = Transcript(agent: "Claude")
        transcript.entries = texts.enumerated().map { index, text in
            Entry(id: EntryId(UInt64(index + 1)), kind: .agent(text: text))
        }
        return transcript
    }

    /// Every entry of `texts` changed, as a live update sends them.
    private func live(_ effects: inout ChatEffects, _ texts: [String], at now: Date) {
        let transcript = transcript(texts)
        effects.live(transcript, changed: transcript.entries.map(\.id), now: now)
    }

    private func key(_ entry: UInt64, _ order: Int) -> ChatEffects.BubbleKey {
        .init(entry: EntryId(entry), order: order)
    }

    @Test func aLoadedTranscriptNotesItsEffectsWithoutPlayingThem() {
        var effects = ChatEffects()
        let texts = [":::slam\nhi\n:::\n::confetti"]
        effects.history(transcript(texts))
        #expect(effects.playing(at: now) == nil)
        #expect(effects.bubbleSince(key(1, 0)) == nil)
        // Seen again, as a reconnect sends it, it still does not play.
        live(&effects, texts, at: now)
        #expect(effects.playing(at: now) == nil)
        #expect(effects.bubbleSince(key(1, 0)) == nil)
    }

    @Test func aNewMessagePlaysItsEffectsOnce() {
        var effects = ChatEffects()
        effects.history(transcript(["earlier"]))
        let texts = ["earlier", ":::slam\nFound it\n:::\n::confetti"]
        live(&effects, texts, at: now)
        let playing = effects.playing(at: now)
        #expect(playing?.entry == EntryId(2))
        #expect(playing?.effect == .confetti)
        #expect(playing?.text == "Found it")
        #expect(effects.bubbleSince(key(2, 0)) == now)
        // The next chunk of the same message does not start them again.
        let later = now.addingTimeInterval(0.1)
        live(&effects, texts, at: later)
        #expect(effects.playing(at: later)?.since == now)
        #expect(effects.bubbleSince(key(2, 0)) == now)
        // Once over, it stays over.
        #expect(effects.playing(at: now.addingTimeInterval(ScreenEffect.confetti.length)) == nil)
    }

    @Test func aBubbleThatStreamsInLaterPlaysOnItsOwn() {
        var effects = ChatEffects()
        let later = now.addingTimeInterval(0.5)
        live(&effects, [":::slam\nOne\n:::\nthen"], at: now)
        live(&effects, [":::slam\nOne\n:::\nthen\n:::loud\nTwo"], at: later)
        #expect(effects.bubbleSince(key(1, 0)) == now)
        #expect(effects.bubbleSince(key(1, 1)) == later)
    }

    @Test func aScreenEffectArrivingWhileOnePlaysIsDropped() {
        var effects = ChatEffects()
        live(&effects, ["::confetti\nOne"], at: now)
        let soon = now.addingTimeInterval(0.2)
        live(&effects, ["::confetti\nOne", "::lasers\nTwo"], at: soon)
        #expect(effects.playing(at: soon)?.effect == .confetti)
        // It is not queued: the confetti ends and nothing follows.
        #expect(effects.playing(at: now.addingTimeInterval(ScreenEffect.confetti.length)) == nil)
        live(&effects, ["::confetti\nOne", "::lasers\nTwo"], at: now.addingTimeInterval(10))
        #expect(effects.playing(at: now.addingTimeInterval(10)) == nil)
    }

    @Test func aReplayPlaysAnEffectAgain() {
        var effects = ChatEffects()
        let transcript = transcript([":::loud\nHey\n:::\n::love"])
        effects.history(transcript)
        let later = now.addingTimeInterval(30)
        effects.replayBubble(key(1, 0), now: later)
        #expect(effects.bubbleSince(key(1, 0)) == later)
        effects.replayScreen(transcript.entries[0], now: later)
        #expect(effects.playing(at: later)?.effect == .love)
    }

    @Test func anEffectFollowedByManyEntriesInOneUpdateStillPlays() {
        var effects = ChatEffects()
        effects.history(transcript(["earlier"]))
        live(&effects, ["earlier", ":::loud\nDone\n:::\n::confetti"] + Array(repeating: "step", count: 8), at: now)
        #expect(effects.playing(at: now)?.entry == EntryId(2))
        #expect(effects.bubbleSince(key(2, 0)) == now)
    }

    @Test func onlyTheEntriesAnUpdateChangedAreLookedAt() {
        var effects = ChatEffects()
        let transcript = transcript(["::confetti\nOne", "two"])
        effects.live(transcript, changed: [EntryId(2)], now: now)
        #expect(effects.playing(at: now) == nil)
        effects.live(transcript, changed: [EntryId(1)], now: now)
        #expect(effects.playing(at: now)?.entry == EntryId(1))
    }

    @Test func theWordsAnEffectDrawsFollowItsMessageAsItStreams() {
        var effects = ChatEffects()
        live(&effects, ["::echo\n"], at: now)
        #expect(effects.playing(at: now)?.text == "")
        live(&effects, ["::echo\nMerged!"], at: now.addingTimeInterval(0.1))
        let playing = effects.playing(at: now)
        #expect(playing?.text == "Merged!")
        #expect(playing?.since == now)
    }

    @Test func aUserMessageNeverPlaysAnEffect() {
        var effects = ChatEffects()
        var transcript = Transcript(agent: "Claude")
        transcript.entries = [Entry(id: 1, kind: .user(text: "::confetti\n:::slam\nme\n:::", files: 0))]
        effects.live(transcript, changed: [EntryId(1)], now: now)
        #expect(effects.playing(at: now) == nil)
        #expect(effects.bubbleSince(key(1, 0)) == nil)
    }

    @Test func endedEffectsAreForgottenAndTheNextEndIsKnown() {
        var effects = ChatEffects()
        live(&effects, [":::slam\nOne\n:::\n::love"], at: now)
        #expect(effects.nextEnd(after: now) == now.addingTimeInterval(ScreenEffect.love.length))
        let after = now.addingTimeInterval(BubbleEffect.longest + ScreenEffect.love.length)
        effects.expire(now: after)
        #expect(effects.bubbleSince(key(1, 0)) == nil)
        #expect(effects.playing(at: after) == nil)
        #expect(effects.nextEnd(after: after) == nil)
    }

    @Test func changesNameTheEntriesTheyTouch() {
        let entry = Entry(id: 4, kind: .agent(text: "x"))
        #expect(Change.push(entry).entry == EntryId(4))
        #expect(Change.replace(entry).entry == EntryId(4))
        #expect(Change.append(id: 4, text: "y").entry == EntryId(4))
        #expect(Change.status(.idle).entry == nil)
    }
}
