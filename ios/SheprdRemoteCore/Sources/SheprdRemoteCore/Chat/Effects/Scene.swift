import CoreGraphics
import Foundation

/// What a screen effect draws at a moment: boxes, beams, words, and a veil,
/// laid out on the chat screen (`chat/effects/scene.rs`). Each frame is a
/// function of the time since the effect began alone, so a frame never
/// depends on the one before and a test can look at any moment. Particles
/// scatter by a fixed hash, so an effect plays the same way each time.
public enum EffectScene {
    /// Bright colors an effect cycles through.
    public static let brights = 6
    /// Golds a celebration's sparks take.
    public static let golds = 4

    /// A color, by meaning: the view picks it.
    public enum Hue: Hashable, Sendable {
        /// One of the bright colors, by its order.
        case bright(Int)
        /// One of a celebration's golds.
        case gold(Int)
        case white
    }

    /// Words an effect draws.
    public enum Words: Hashable, Sendable {
        /// The message's own words.
        case message
        case heart
        case smallHeart
        /// One letter of the message's: the `index`th that is not a space,
        /// standing in a formation's `row`.
        case letter(index: Int, row: Int)
    }

    public enum Mark: Hashable, Sendable {
        /// A filled box, rounded by `round`: a confetto, a spark, or a balloon.
        case box(CGRect, round: Double, hue: Hue, alpha: Double)
        /// A straight line, such as a laser or a balloon's string.
        case beam(from: CGPoint, to: CGPoint, width: Double, hue: Hue, alpha: Double)
        /// Words centered on `x`, with their top at `y`.
        case words(x: Double, y: Double, size: Double, alpha: Double, words: Words)
        /// The screen dimmed, but for the hole, if any.
        case veil(alpha: Double, hole: CGRect?)
    }

    /// What `effect` draws `t` seconds in over a screen of `size`; the
    /// message that asked for it sits at `target`, when it shows, and has
    /// `letters` letters that are not spaces.
    public static func marks(
        _ effect: ScreenEffect,
        at t: Double,
        size: CGSize,
        target: CGRect? = nil,
        letters: Int = 0
    ) -> [Mark] {
        let length = effect.length
        guard t >= 0, t < length, size.width > 0, size.height > 0 else { return [] }
        let (w, h) = (Double(size.width), Double(size.height))
        var marks: [Mark] = []
        switch effect {
        case .balloons: balloons(t, w, h, &marks)
        case .celebration: celebration(t, length, w, &marks)
        case .confetti: confetti(t, length, w, h, &marks)
        case .echo: echo(t, w, h, &marks)
        case .fireworks: fireworks(t, length, w, h, &marks)
        case .lasers: lasers(t, length, w, h, &marks)
        case .love: love(t, length, w, h, &marks)
        case .spotlight: spotlight(t, length, w, h, target, &marks)
        case .zap: Zap.marks(t, length, w, h, letters, &marks)
        }
        return marks
    }

    /// The letters of `text` that are not spaces: the targets zap lines up.
    public static func letters(in text: String) -> Int {
        text.count { !$0.isWhitespace }
    }

    /// The `index`th letter of `text` that is not a space, or nothing.
    public static func letter(of text: String, at index: Int) -> String {
        text.lazy.filter { !$0.isWhitespace }.dropFirst(index).first.map(String.init) ?? ""
    }

    /// A number in `0..<1` the particle `index` always draws for `salt`.
    static func unit(_ index: UInt32, _ salt: UInt32) -> Double {
        var x = index &* 0x9E37_79B9 &+ (salt &* 0x85EB_CA6B)
        x ^= x >> 16
        x = x &* 0x7FEB_352D
        x ^= x >> 15
        x = x &* 0x846C_A68B
        x ^= x >> 16
        return Double(x >> 8) / Double(1 << 24)
    }

    static func between(_ index: UInt32, _ salt: UInt32, _ low: Double, _ high: Double) -> Double {
        low + unit(index, salt) * (high - low)
    }

    /// How much an effect shows `t` seconds in: fading in over `rise` and
    /// out over the last `fall` of its `length`.
    static func envelope(_ t: Double, _ length: Double, _ rise: Double, _ fall: Double) -> Double {
        min(max(min(t / rise, (length - t) / fall), 0), 1)
    }

    /// Where invisible ink's speck `index` sits for `frame`, as fractions of
    /// the text's width and height, and how dark it is.
    public static func speck(_ index: UInt32, frame: UInt32) -> (x: Double, y: Double, alpha: Double) {
        let salt = frame &* 3
        return (unit(index, salt &+ 11), unit(index, salt &+ 12), between(index, salt &+ 13, 0.25, 1))
    }

    private static func box(_ x: Double, _ y: Double, _ w: Double, _ h: Double, round: Double, hue: Hue, alpha: Double) -> Mark {
        .box(CGRect(x: x, y: y, width: w, height: h), round: round, hue: hue, alpha: alpha)
    }

    private static func confetti(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        let alpha = envelope(t, length, 0.05, 0.6)
        for i in UInt32(0)..<150 {
            let phase = between(i, 1, 0, 2 * .pi)
            let speed = h * between(i, 2, 0.45, 0.8)
            let x = between(i, 3, 0, w) + between(i, 4, -40, 40) * t + sin(t * 3 + phase) * 24
            let y = between(i, 5, -0.6, 0) * h - 12 + speed * t
            // A confetto flips as it falls: its height follows the flip.
            let (pw, ph) = (between(i, 6, 5, 9), between(i, 7, 8, 14))
            let flip = max(abs(cos(t * 6 + phase)), 0.12)
            marks.append(box(x - pw / 2, y - ph * flip / 2, pw, ph * flip, round: 1, hue: .bright(Int(i) % brights), alpha: alpha))
        }
    }

    private static func balloons(_ t: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        for i in UInt32(0)..<14 {
            let size = between(i, 1, 22, 34)
            let speed = h * between(i, 2, 0.42, 0.6)
            let x = between(i, 3, 24, max(w - 24, 24)) + sin(t * 1.5 + between(i, 4, 0, 2 * .pi)) * 12
            let y = h + size + between(i, 5, 0, 0.5) * h - speed * t
            marks.append(.beam(from: CGPoint(x: x, y: y + size), to: CGPoint(x: x + 4, y: y + size * 3), width: 1, hue: .white, alpha: 0.5))
            marks.append(box(x - size * 0.82, y - size, size * 1.64, size * 2, round: size * 0.82, hue: .bright(Int(i) % brights), alpha: 0.95))
            // The shine on its upper left.
            marks.append(box(x - size * 0.48, y - size * 0.7, size * 0.36, size * 0.6, round: size * 0.18, hue: .white, alpha: 0.35))
        }
    }

    private static func fireworks(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        marks.append(.veil(alpha: 0.55 * envelope(t, length, 0.4, 0.5), hole: nil))
        let rocketTime = 1.1
        for rocket in UInt32(0)..<9 {
            let launch = Double(rocket) * 0.33
            let x = w * between(rocket, 1, 0.2, 0.8)
            let top = h * between(rocket, 2, 0.15, 0.45)
            let flight = rocketTime * (h - top) / h
            let since = t - launch
            if since < 0 { break }
            if since < flight {
                let y = h - (h - top) * since / flight
                marks.append(box(x - 1, y, 2, 9, round: 1, hue: .white, alpha: 0.9))
                continue
            }
            let burst = since - flight
            let life = 1 - burst * 0.8
            if life <= 0 { continue }
            for spark in UInt32(0)..<64 {
                let id = rocket * 64 + spark
                let angle = between(id, 3, 0, 2 * .pi)
                let speed = between(id, 4, 40, 170)
                let sx = x + cos(angle) * speed * burst
                let sy = top + sin(angle) * speed * burst + 45 * burst * burst
                marks.append(box(sx - 2, sy - 2, 4, 4, round: 2, hue: .bright(Int(rocket) % brights), alpha: life))
            }
        }
    }

    private static func lasers(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        let fade = envelope(t, length, 0.3, 0.3)
        marks.append(.veil(alpha: 0.75 * fade, hole: nil))
        let center = CGPoint(x: w / 2, y: h * 0.45)
        let reach = max(w, h) * 1.5
        for i in 0..<10 {
            let turn = i % 2 == 0 ? -1.1 : 1.3
            let angle = t * turn + Double(i) * 0.63
            let to = CGPoint(x: center.x + cos(angle) * reach, y: center.y + sin(angle) * reach)
            // A wide faint beam under a narrow bright one reads as a glow.
            for (width, alpha) in [(9.0, 0.2), (3.0, 0.85)] {
                marks.append(.beam(from: center, to: to, width: width, hue: .bright(i % brights), alpha: alpha * fade))
            }
        }
        marks.append(.words(x: center.x, y: center.y - 14, size: 22, alpha: fade, words: .message))
    }

    private static func celebration(_ t: Double, _ length: Double, _ w: Double, _ marks: inout [Mark]) {
        let sparks: UInt32 = 240
        let pouring = 2.4
        for i in UInt32(0)..<sparks {
            let born = Double(i) * pouring / Double(sparks)
            let age = t - born
            if age < 0 { break }
            let life = min(1 - age * 0.55, (length - t) / 0.4)
            if life <= 0 { continue }
            // Down and to the left, out of the top right corner.
            let angle = between(i, 1, .pi * 0.5, .pi * 1.05)
            let speed = between(i, 2, 120, 380)
            let x = w + cos(angle) * speed * age
            let y = abs(sin(angle)) * speed * age + 20 * age * age
            let size = between(i, 3, 2, 6.4)
            marks.append(box(x - size / 2, y - size / 2, size, size, round: size / 2, hue: .gold(Int(i) % golds), alpha: life))
        }
    }

    private static func echo(_ t: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        for i in UInt32(0)..<28 {
            let delay = Double(i) * 0.06
            let span = between(i, 1, 1.8, 3)
            let k = (t - delay) / span
            guard k >= 0, k < 1 else { continue }
            let eased = 1 - pow(1 - k, 3)
            let alpha = 0.9 * min(max(min(k / 0.15, (1 - k) / 0.2), 0), 1)
            marks.append(.words(
                x: between(i, 2, 0.05, 0.95) * w,
                y: h + 20 - (h + 100) * eased,
                size: between(i, 3, 12, 30),
                alpha: alpha,
                words: .message
            ))
        }
    }

    private static func spotlight(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ target: CGRect?, _ marks: inout [Mark]) {
        // A message spans the column, but its words start at its left:
        // light that part, no wider than half the screen.
        let lit = target.map { CGRect(x: $0.minX, y: $0.minY, width: min($0.width, w * 0.5), height: $0.height) }
        let (tx, ty) = lit.map { (Double($0.midX), Double($0.midY)) } ?? (w / 2, h / 2)
        let (tw, th) = lit.map { (Double($0.width) + 60, Double($0.height) + 40) } ?? (160, 60)
        let k = min(t / 1.2, 1)
        let eased = 1 - pow(1 - k, 3)
        // It sweeps in from the upper left as a small round light, wavering
        // until it settles on the message.
        let x = w * 0.15 + (tx - w * 0.15) * eased + sin(t * 4) * 30 * (1 - eased)
        let y = h * 0.2 + (ty - h * 0.2) * eased
        let (rw, rh) = (120 + (tw - 120) * eased, 90 + (th - 90) * eased)
        marks.append(.veil(
            alpha: 0.78 * envelope(t, length, 0.25, 0.5),
            hole: CGRect(x: x - rw / 2, y: y - rh / 2, width: rw, height: rh)
        ))
    }

    private static func love(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ marks: inout [Mark]) {
        let fade = envelope(t, length, 0.05, 0.5)
        for i in UInt32(0)..<26 {
            let speed = between(i, 1, 50, 110)
            marks.append(.words(
                x: between(i, 2, 0, w) + sin(t * 2 + between(i, 3, 0, 2 * .pi)) * 10,
                y: h + between(i, 4, 0, 200) - speed * t,
                size: between(i, 5, 10, 20),
                alpha: fade,
                words: .smallHeart
            ))
        }
        // The heart beats twice, then rises away.
        let k = t / length
        let beat = switch k {
        case ..<0.2: 1.1 * k / 0.2
        case ..<0.3: 1.1 - 0.15 * (k - 0.2) / 0.1
        case ..<0.4: 0.95 + 0.2 * (k - 0.3) / 0.1
        case ..<0.55: 1.15 - 0.15 * (k - 0.4) / 0.15
        default: 1.0
        }
        let rise = max((k - 0.55) / 0.45, 0) * h * 0.2
        let size = 120 * beat
        marks.append(.words(x: w / 2, y: h * 0.45 - size / 2 - rise, size: size, alpha: k < 0.85 ? 1 : min((1 - k) / 0.15, 1), words: .heart))
    }
}
