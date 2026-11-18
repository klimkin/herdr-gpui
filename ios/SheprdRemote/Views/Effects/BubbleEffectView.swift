import SheprdRemoteCore
import SwiftUI

/// Markdown an agent wrapped in a bubble effect, in the pose the effect has
/// now, with the link that plays it again. Slam, loud, and gentle scale,
/// fade, and shake the text while they play; invisible ink hides it under
/// specks until tapped, and a second tap hides it again.
struct BubbleEffectView<Content: View>: View {
    let effect: BubbleEffect
    /// When it began to play, while it may still be playing.
    let since: Date?
    /// Plays it again; `nil` shows no link.
    var replay: (() -> Void)?
    /// A moment to hold it at instead of the clock's, for mockups.
    var held: Double?
    @ViewBuilder var content: Content
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Whether invisible ink shows its words. Kept here, above the moving
    /// and resting branches, so ink revealed while it shimmers stays
    /// revealed when the shimmer ends.
    @State private var revealed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let held {
                posed(at: held)
            } else if let since, !reduceMotion {
                TimelineView(.animation) { timeline in
                    posed(at: max(0, timeline.date.timeIntervalSince(since)))
                }
            } else {
                posed(at: nil)
            }
            if effect != .ink, let replay {
                ReplayLink(label: effect.label, action: replay)
            }
        }
    }

    @ViewBuilder
    private func posed(at t: Double?) -> some View {
        let pose = effect.pose(at: t)
        if effect == .ink {
            InkCover(frame: pose.shimmer ? UInt32((t ?? 0) * Ink.shimmerRate) + 1 : 0, revealed: $revealed) { content }
        } else {
            content
                .scaleEffect(pose.scale, anchor: .leading)
                .opacity(pose.opacity)
                .offset(x: pose.shift)
        }
    }
}

/// How invisible ink's specks are scattered.
enum Ink {
    /// How often the specks move while they shimmer, per second.
    static let shimmerRate = 14.0
    /// Specks per square point of the text they hide.
    static let density = 1.0 / 18
    /// The most specks one bubble draws.
    static let maxSpecks = 2_400
}

/// Text under invisible ink: specks hide it until tapped. `frame` scatters
/// the specks; it stays put once the effect is over, so a hidden message
/// costs no frames.
struct InkCover<Content: View>: View {
    let frame: UInt32
    @Binding var revealed: Bool
    @ViewBuilder var content: Content

    var body: some View {
        content
            .opacity(revealed ? 1 : 0)
            .overlay {
                Canvas { context, size in
                    let count = min(Int(size.width * size.height * Ink.density), Ink.maxSpecks)
                    for index in 0..<UInt32(count) {
                        let speck = EffectScene.speck(index, frame: frame)
                        let rect = CGRect(x: speck.x * size.width, y: speck.y * size.height, width: 1.5, height: 1.5)
                        context.fill(Path(rect), with: .color(.primary.opacity(speck.alpha * 0.85)))
                    }
                }
                .opacity(revealed ? 0 : 1)
                .allowsHitTesting(false)
            }
            .contentShape(.rect)
            .onTapGesture { withAnimation(.easeInOut(duration: 0.3)) { revealed.toggle() } }
            .accessibilityElement(children: revealed ? .contain : .ignore)
            .accessibilityLabel(revealed ? Text("") : Text("Hidden with invisible ink"))
            .accessibilityHint(revealed ? Text("Double-tap to hide it again") : Text("Double-tap to reveal"))
            .accessibilityAddTraits(.isButton)
    }
}

/// The link under a message that plays its effect again.
struct ReplayLink: View {
    let label: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label("Replay \(label)", systemImage: "arrow.clockwise")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Replay \(label)")
    }
}
