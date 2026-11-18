import SheprdRemoteCore
import SwiftUI

// Text sizes and message effects in the chat, drawn by the app's own
// effect views (`SheprdRemote/Views/Effects`) at fixed moments. Sizes: the
// steps in prose, a prompt, and a table. Bubbles 0: slam, loud, and gentle
// partway through, ink hiding its words; 1: all at rest with replay links.
// Screen 0–3: confetti, zap, spotlight, and fireworks partway through.

private struct EffectsChat<Content: View>: View {
    var prompt = "Run the release checks and tell me if we can :big[ship]."
    @ViewBuilder var content: Content

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    HStack(alignment: .top, spacing: 10) {
                        Rectangle().fill(.tint).frame(width: 3)
                        SizedPlain(text: prompt, style: .title3).font(.title3.weight(.semibold))
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    content
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20)
                .padding(.top, 16)
            }
            .navigationTitle("Release checks")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

struct EffectsSizes: View {
    var body: some View {
        EffectsChat {
            SizedMarkdown(text: "Steps: :small[small] prose :big[big] :huge[huge] :giant[giant]")
            SizedMarkdown(text: "Tests :big[pass]: :huge[214 of 214]. :small[CI took 4 min, and the cache was warm.]")
            SizedMarkdown(text: "The resize bug was a **stale cell width**; :big[the fix is *one line*] in `layout.rs`.")
            SizedMarkdown(text: ":huge[Do not merge] until the migration runs on staging.")
            SizedMarkdown(text: "Written as is: `:big[code]`, :large[unknown], and http://host:big[address].", style: .body)
                .foregroundStyle(.secondary)
        }
    }
}

struct EffectsBubbles: View {
    var state: Int

    var body: some View {
        EffectsChat {
            bubble(.slam, at: 0.35, "Found it: the resize bug was a **stale cell width**.")
            bubble(.loud, at: 0.5, "Do not merge before the migration runs.")
            bubble(.gentle, at: 0.8, "Take a break; the rest can wait until tomorrow.")
            bubble(.ink, at: 1.2, "The release code word is *marmalade*.")
            Label("Replay Confetti", systemImage: "arrow.clockwise").font(.caption).foregroundStyle(.secondary)
        }
    }

    private func bubble(_ effect: BubbleEffect, at t: Double, _ text: String) -> some View {
        BubbleEffectView(effect: effect, since: nil, replay: {}, held: state == 0 ? t : nil) {
            SizedMarkdown(text: text)
        }
    }
}

struct EffectsScreen: View {
    var state: Int
    /// Where the reply is drawn, measured as the chat measures it.
    @State private var target: CGRect?

    private var effect: (ScreenEffect, Double) {
        switch state {
        case 1: (.zap, 1.6)
        case 2: (.spotlight, 2)
        case 3: (.fireworks, 2.2)
        default: (.confetti, 1.1)
        }
    }

    var body: some View {
        let reply = "All checks pass and the PR is merged."
        let (effect, t) = effect
        EffectsChat {
            SizedMarkdown(text: "Tests :big[pass]: :huge[214 of 214]. :small[CI took 4 min.]")
            VStack(alignment: .leading, spacing: 10) {
                SizedMarkdown(text: "All checks pass and :big[the PR is merged].")
                Label("Replay \(effect.label)", systemImage: "arrow.clockwise").font(.caption).foregroundStyle(.secondary)
            }
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .named("effects")) } action: { target = $0 }
        }
        .overlay {
            ScreenEffectCanvas(effect: effect, text: reply, t: t, target: target)
                .ignoresSafeArea()
        }
        .coordinateSpace(.named("effects"))
    }
}
