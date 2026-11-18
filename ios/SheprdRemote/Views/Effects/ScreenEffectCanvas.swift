import SheprdRemoteCore
import SwiftUI

/// A screen effect drawn over the chat (`chat/view/effects.rs` on the
/// desktop): the marks `EffectScene` lays out for the moment `t`, painted
/// with the system's bright colors. It takes no touches, so the chat stays
/// usable while it plays.
struct ScreenEffectCanvas: View {
    let effect: ScreenEffect
    /// The message's words, for the effects that draw them.
    let text: String
    let t: Double
    /// Where the message that asked for it is, for a spotlight.
    var target: CGRect?

    /// Bright colors by their order in the scene: red, green, yellow, blue,
    /// magenta, and cyan, as a terminal theme orders them.
    static let brights: [Color] = [.red, .green, .yellow, .blue, Color(red: 0.85, green: 0.3, blue: 0.85), .cyan]
    /// Golds a celebration's sparks take, whatever the appearance.
    static let golds: [Color] = [
        Color(red: 1, green: 0.843, blue: 0.416),
        Color(red: 1, green: 0.914, blue: 0.659),
        Color(red: 1, green: 0.761, blue: 0.239),
        Color(red: 1, green: 0.957, blue: 0.839),
    ]
    /// Zap's rows: yellow, cyan, and magenta, which read on dark and light.
    static let rowHues = [2, 5, 4]

    var body: some View {
        Canvas { context, size in
            let marks = EffectScene.marks(effect, at: t, size: size, target: target, letters: EffectScene.letters(in: text))
            for mark in marks { draw(mark, in: &context, size: size) }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func color(_ hue: EffectScene.Hue) -> Color {
        switch hue {
        case let .bright(index): Self.brights[index % Self.brights.count]
        case let .gold(index): Self.golds[index % Self.golds.count]
        case .white: .white
        }
    }

    /// A heart centered on `x` with its top at `y`: a symbol rather than an
    /// emoji, which a canvas does not draw.
    private func drawHeart(_ color: Color, x: Double, y: Double, size: Double, alpha: Double, in context: GraphicsContext) {
        var layer = context
        layer.opacity = alpha
        var heart = layer.resolve(Image(systemName: "heart.fill"))
        heart.shading = .color(color)
        layer.draw(heart, in: CGRect(x: x - size / 2, y: y, width: size, height: size * 0.9))
    }

    private func draw(_ mark: EffectScene.Mark, in context: inout GraphicsContext, size: CGSize) {
        switch mark {
        case let .box(rect, round, hue, alpha):
            let shape = round > 0 ? Path(roundedRect: rect, cornerRadius: min(round, min(rect.width, rect.height) / 2)) : Path(rect)
            context.fill(shape, with: .color(color(hue).opacity(alpha)))
        case let .beam(from, to, width, hue, alpha):
            var path = Path()
            path.move(to: from)
            path.addLine(to: to)
            context.stroke(path, with: .color(color(hue).opacity(alpha)), style: StrokeStyle(lineWidth: width, lineCap: .round))
        case let .words(x, y, size, alpha, .heart):
            drawHeart(.red, x: x, y: y, size: size, alpha: alpha, in: context)
        case let .words(x, y, size, alpha, .smallHeart):
            drawHeart(.pink, x: x, y: y, size: size, alpha: alpha, in: context)
        case let .words(x, y, size, alpha, words):
            let (string, style): (String, AnyShapeStyle) = switch words {
            case .message: (text, effect == .lasers ? AnyShapeStyle(Color.white) : AnyShapeStyle(.primary))
            case .heart, .smallHeart: ("", AnyShapeStyle(Color.white))
            case let .letter(index, row):
                (EffectScene.letter(of: text, at: index), AnyShapeStyle(Self.brights[Self.rowHues[row % Self.rowHues.count]]))
            }
            guard !string.isEmpty else { return }
            var layer = context
            layer.opacity = alpha
            let text = Text(string).font(.system(size: size, weight: .semibold)).foregroundStyle(style)
            layer.draw(text, at: CGPoint(x: x, y: y), anchor: .top)
        case let .veil(alpha, hole):
            var path = Path(CGRect(origin: .zero, size: size))
            if let hole {
                path.addPath(Path(roundedRect: hole, cornerRadius: min(hole.width, hole.height) / 2))
            }
            context.fill(path, with: .color(.black.opacity(alpha)), style: FillStyle(eoFill: true))
        }
    }
}

/// The screen effect playing, if any, each frame a function of the time
/// since it began.
struct ScreenEffectOverlay: View {
    let playing: ChatEffects.Playing?
    /// Where the message that asked for it was last drawn.
    var target: (ChatEffects.Playing) -> CGRect? = { _ in nil }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if let playing, !reduceMotion {
            TimelineView(.animation) { timeline in
                if let t = playing.elapsed(at: timeline.date) {
                    ScreenEffectCanvas(effect: playing.effect, text: playing.text, t: t, target: target(playing))
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
    }
}
