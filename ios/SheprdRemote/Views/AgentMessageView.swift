import SheprdRemoteCore
import SwiftUI

/// An agent's reply: its Markdown, the bubbles it wraps in effects, and,
/// for a reply that asked for a screen effect, the link that plays it
/// again. The open chat's `ChatEffectsModel` says what plays now; without
/// one, every effect shows at rest.
struct AgentMessageView: View {
    let id: EntryId
    let text: String
    @Environment(\.chatEffects) private var effects

    var body: some View {
        let message = AgentMessage.parse(text)
        let bubbles = bubbleOrders(message)
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(message.parts.enumerated()), id: \.offset) { index, part in
                switch part {
                case let .markdown(blocks):
                    MarkdownView(blocks: blocks)
                case let .bubble(effect, blocks):
                    let key = ChatEffects.BubbleKey(entry: id, order: bubbles[index] ?? 0)
                    BubbleEffectView(
                        effect: effect,
                        since: effects?.ledger.bubbleSince(key),
                        replay: effects.map { effects in { effects.replayBubble(key) } }
                    ) {
                        MarkdownView(blocks: blocks)
                    }
                }
            }
            if let screen = message.screen, let effects {
                ReplayLink(label: screen.label) {
                    effects.replayScreen(Entry(id: id, kind: .agent(text: text)))
                }
            }
        }
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(ChatEffectsModel.space)) } action: { frame in
            effects?.frames[id] = frame
        }
    }

    /// Each bubble part's order among the message's bubbles, by its index
    /// among the parts.
    private func bubbleOrders(_ message: AgentMessage) -> [Int: Int] {
        var orders: [Int: Int] = [:]
        for (index, part) in message.parts.enumerated() {
            if case .bubble = part { orders[index] = orders.count }
        }
        return orders
    }
}
