#if DEBUG
import Foundation
import SheprdRemoteCore

// A made-up chat for the simulator loop's `-sampleChat effects`: text sizes
// and message effects, for screenshots. `-samplePlay NAME` asks for the
// screen effect NAME in the last reply and plays it and every bubble again,
// as a live reply would.

extension PaneKey {
    static let effectsSample = PaneKey(device: "sample", pane: "effects")
}

extension Transcript {
    static func effectsSample(screen: ScreenEffect = .confetti) -> Transcript {
        var transcript = Transcript(agent: "Claude Code")
        transcript.status = .idle
        transcript.entries = [
            Entry(id: 1, kind: .user(text: "Run the release checks and tell me if we can :big[ship].", files: 0)),
            Entry(id: 2, kind: .agent(text: """
            Sizes: :small[small] prose :big[big] :huge[huge] :giant[giant]

            Tests :big[pass]: :huge[214 of 214]. :small[CI took 4 min.]
            """)),
            Entry(id: 3, kind: .agent(text: """
            :::slam
            Found it: the resize bug was a **stale cell width**.
            :::

            :::ink
            The release code word is *marmalade*.
            :::

            :::loud
            Do not merge before the migration runs.
            :::
            """)),
            Entry(id: 4, kind: .agent(text: """
            ::\(screen.rawValue)
            All checks pass and :big[the PR is merged].
            """)),
        ]
        return transcript
    }
}

extension ChatStore {
    /// Plays the sample's effects again, as `-samplePlay NAME` asks.
    func playSample(_ name: String) {
        guard let transcript else { return }
        for entry in transcript.entries {
            guard case let .agent(text) = entry.kind else { continue }
            let message = AgentMessage.parse(text)
            for order in 0..<message.bubbles {
                effects.replayBubble(ChatEffects.BubbleKey(entry: entry.id, order: order))
            }
            if message.screen?.rawValue == name { effects.replayScreen(entry) }
        }
    }
}
#endif
