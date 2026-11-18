import Foundation
import SheprdRemoteCore
import SwiftUI

/// One open chat's message effects (`ChatEffects`), observed by its views:
/// what plays now, and when each ends, after which the views settle and
/// nothing redraws for it.
@MainActor
@Observable
final class ChatEffectsModel {
    /// The coordinate space the chat's scroll view names, which the screen
    /// effect is drawn in and the spotlight's message is measured in.
    nonisolated static let space = "chat-effects"

    private(set) var ledger = ChatEffects()
    /// Where each reply was last drawn, for the spotlight to find its own.
    /// Read each frame by the effect, so a change is no reason to redraw.
    @ObservationIgnored var frames: [EntryId: CGRect] = [:]
    @ObservationIgnored private var expiry: Task<Void, Never>?

    /// Notes a whole transcript's effects without playing any.
    func history(_ transcript: Transcript) {
        update { $0.history(transcript) }
    }

    /// Plays what is new in the entries a live update changed.
    func live(_ transcript: Transcript, changed: [EntryId]) {
        guard !changed.isEmpty else { return }
        update { $0.live(transcript, changed: changed, now: .now) }
    }

    func replayBubble(_ key: ChatEffects.BubbleKey) {
        update { $0.replayBubble(key, now: .now) }
    }

    func replayScreen(_ entry: Entry) {
        update { $0.replayScreen(entry, now: .now) }
    }

    /// Where the message a playing screen effect belongs to was last drawn.
    func target(of playing: ChatEffects.Playing) -> CGRect? {
        frames[playing.entry]
    }

    /// Applies `change` to the ledger, assigning it only when it changed so
    /// a streamed chunk that starts nothing redraws no message.
    private func update(_ change: (inout ChatEffects) -> Void) {
        var next = ledger
        change(&next)
        if next != ledger { ledger = next }
        scheduleExpiry()
    }

    /// Wakes when the next effect ends, to forget it.
    private func scheduleExpiry() {
        expiry?.cancel()
        guard let end = ledger.nextEnd(after: .now) else { return }
        expiry = Task { [weak self] in
            try? await Task.sleep(for: .seconds(max(0, end.timeIntervalSinceNow) + 0.05))
            guard !Task.isCancelled, let self else { return }
            self.update { $0.expire(now: .now) }
        }
    }
}

extension EnvironmentValues {
    /// The open chat's effects, for the messages that play them; `nil`
    /// draws every effect at rest without replay links.
    @Entry var chatEffects: ChatEffectsModel?
}
