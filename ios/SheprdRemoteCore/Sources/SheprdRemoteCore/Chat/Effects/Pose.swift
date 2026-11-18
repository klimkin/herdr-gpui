import Foundation

/// How a bubble effect holds its Markdown at a moment: how large, how
/// opaque, and how far it shakes sideways (`chat/effects/pose.rs`). Like a
/// screen effect's scene, it is a function of the time since the effect
/// began alone.
public struct Pose: Hashable, Sendable {
    /// The text's size, as a multiple of its own.
    public var scale: Double
    public var opacity: Double
    /// How far it sits sideways, in points.
    public var shift: Double
    /// Whether invisible ink's specks still move.
    public var shimmer: Bool

    public static let rest = Pose(scale: 1, opacity: 1, shift: 0, shimmer: false)

    public init(scale: Double, opacity: Double, shift: Double, shimmer: Bool) {
        self.scale = scale
        self.opacity = opacity
        self.shift = shift
        self.shimmer = shimmer
    }
}

/// The largest a slam starts.
private let slamFrom = 2.2
/// How much larger loud text grows.
private let loudScale = 1.45

/// Eases `from` to `to` as `k` goes from `start` to `end`.
private func tween(_ k: Double, _ start: Double, _ end: Double, _ from: Double, _ to: Double) -> Double {
    let k = min(max((k - start) / (end - start), 0), 1)
    return from + (to - from) * (1 - (1 - k) * (1 - k))
}

extension BubbleEffect {
    /// The pose `t` seconds in, or at rest once the effect is over or never
    /// played.
    public func pose(at t: Double?) -> Pose {
        guard let t, t >= 0, t < length else { return .rest }
        let k = t / length
        switch self {
        case .slam:
            let scale = if k < 0.3 {
                tween(k, 0, 0.3, slamFrom, 0.92)
            } else if k < 0.42 {
                tween(k, 0.3, 0.42, 0.92, 1.04)
            } else {
                tween(k, 0.42, 0.6, 1.04, 1)
            }
            // It lands with a shudder.
            let shake = (0.3..<0.7).contains(k) ? sin(t * 70) * 4 * (0.7 - k) / 0.4 : 0
            return Pose(scale: scale, opacity: min(k / 0.25, 1), shift: shake, shimmer: false)
        case .loud:
            let scale = k < 0.7 ? tween(k, 0, 0.15, 1, loudScale) : tween(k, 0.7, 1, loudScale, 1)
            let shake = (0.15..<0.5).contains(k) ? sin(t * 45) * 2.5 : 0
            return Pose(scale: scale, opacity: 1, shift: shake, shimmer: false)
        case .gentle:
            return Pose(
                scale: k < 0.6 ? tween(k, 0, 0.6, 0.55, 0.6) : tween(k, 0.6, 1, 0.6, 1),
                opacity: k < 0.6 ? tween(k, 0, 0.6, 0.2, 0.8) : tween(k, 0.6, 1, 0.8, 1),
                shift: 0,
                shimmer: false
            )
        case .ink:
            return Pose(scale: 1, opacity: 1, shift: 0, shimmer: true)
        }
    }
}
