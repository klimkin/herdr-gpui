import CoreGraphics
import Foundation

/// Zap (`chat/effects/scene/zap.rs`): the message's letters line up across
/// the top of the screen and sway as a formation, while three pixel ships
/// slide along the bottom and shoot them down one by one; each letter hit
/// bursts into pixels. Each row of the formation takes its own color.
enum Zap {
    /// Letters in the formation; a longer message lends only its first ones.
    static let maxTargets = 24
    /// Letters in a row of the formation, at most: a narrow screen holds fewer.
    static let row = 12
    /// How far apart the letters stand at most, and their rows.
    static let column = 30.0
    /// How far the formation sways to either side at most.
    static let sway = 40.0
    static let line = 38.0
    /// The letters' size.
    static let letter = 22.0
    /// How long a shot takes to reach its letter.
    static let flight = 0.5
    /// How long a letter's burst lasts.
    static let burst = 0.7
    /// When the first and the last letters are hit.
    static let firstHit = 0.8
    static let lastHit = 3.6
    static let ships = 3
    /// A ship, drawn in pixels: a cannon on a wide hull.
    static let ship = ["..#..", "..#..", ".###.", "#####"]
    /// The side of one of a ship's pixels, and of a burst's.
    static let pixel = 4.0
    /// How far the formation sinks each second.
    static let drift = 6.0

    /// How the formation fits a screen: how many letters it holds, how many
    /// in a row, how far apart, and how far it sways, so every letter stays
    /// inside however narrow or short the screen is.
    struct Formation {
        var count: Int
        var perRow: Int
        var column: Double
        var sway: Double

        init(count: Int, w: Double, h: Double) {
            sway = min(w * 0.06, Zap.sway)
            // A letter's half on either side stays on the screen too.
            let room = max(w - 2 * sway - Zap.letter * 2, 0)
            perRow = min(max(Int(room / Zap.column) + 1, 1), Zap.row, max(count, 1))
            column = perRow == 1 ? 0 : min(room / Double(perRow - 1), Zap.column)
            // Rows go down to the ships, allowing for how far the formation
            // sinks before its last letter is hit; a short screen holds fewer.
            let ships = 12 + Double(Zap.ship.count) * Zap.pixel
            let rowRoom = h - Self.top(h) - Zap.drift * Zap.lastHit - ships - Zap.letter
            let rows = Int(max(rowRoom, 0) / Zap.line) + 1
            self.count = min(count, perRow * rows)
        }

        /// Where the first row's letters start.
        static func top(_ h: Double) -> Double { h * 0.12 }

        /// The row letter `index` stands in.
        func row(_ index: Int) -> Int { index / perRow }

        /// Where letter `index` stands at `t`: its center.
        func at(_ index: Int, _ t: Double, _ w: Double, _ h: Double) -> CGPoint {
            let left = w / 2 - Double(perRow - 1) * column / 2
            return CGPoint(
                x: left + Double(index % perRow) * column + sin(t * 1.6) * sway,
                y: Self.top(h) + Double(row(index)) * Zap.line + Zap.drift * t + Zap.letter / 2
            )
        }
    }

    /// Where ship `index`'s cannon is at `t`.
    static func shipAt(_ index: Int, _ t: Double, _ w: Double, _ h: Double) -> CGPoint {
        let home = w * Double(index + 1) / Double(ships + 1)
        return CGPoint(x: home + sin(t * 2 + Double(index) * 2.1) * 60, y: h - 12 - Double(ship.count) * pixel)
    }

    /// When letter `index` is hit: each at its own moment, all before the end.
    static func hitTime(_ index: Int) -> Double {
        EffectScene.between(UInt32(index), 41, firstHit, lastHit)
    }

    static func marks(_ t: Double, _ length: Double, _ w: Double, _ h: Double, _ letters: Int, _ marks: inout [EffectScene.Mark]) {
        let fade = EffectScene.envelope(t, length, 0.2, 0.4)
        // The screen dims, so the formation reads over the conversation.
        marks.append(.veil(alpha: 0.6 * fade, hole: nil))
        let formation = Formation(count: min(letters, maxTargets), w: w, h: h)
        for index in 0..<formation.count {
            let hit = hitTime(index)
            if t < hit {
                let at = formation.at(index, t, w, h)
                marks.append(.words(x: at.x, y: at.y - letter / 2, size: letter, alpha: fade,
                                    words: .letter(index: index, row: formation.row(index))))
            }
            let fired = hit - flight
            if t >= fired, t < hit {
                // From where the ship was when it fired to where the letter
                // is when it is hit.
                let k = (t - fired) / flight
                let from = shipAt(index % ships, fired, w, h)
                let to = formation.at(index, hit, w, h)
                let (x, y) = (from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k)
                marks.append(.box(CGRect(x: x - 1.5, y: y - 5, width: 3, height: 10), round: 1, hue: .white, alpha: fade))
            }
            let since = t - hit
            guard since >= 0, since < burst else { continue }
            let at = formation.at(index, hit, w, h)
            let flash = 1 - since / 0.15
            if flash > 0 {
                marks.append(.box(CGRect(x: at.x - 9, y: at.y - 9, width: 18, height: 18), round: 2, hue: .white, alpha: flash * fade))
            }
            for piece in 0..<12 {
                let id = UInt32(index * 12 + piece)
                let angle = EffectScene.unit(id, 43) * 2 * .pi
                let speed = EffectScene.between(id, 44, 60, 170)
                marks.append(.box(
                    CGRect(
                        x: at.x + cos(angle) * speed * since - pixel / 2,
                        y: at.y + sin(angle) * speed * since + 60 * since * since - pixel / 2,
                        width: pixel,
                        height: pixel
                    ),
                    round: 0,
                    hue: .bright(index % EffectScene.brights),
                    alpha: (1 - since / burst) * fade
                ))
            }
        }
        for index in 0..<ships {
            let at = shipAt(index, t, w, h)
            let half = Double(ship[0].count) * pixel / 2
            for (row, pixels) in ship.enumerated() {
                for (column, pixel) in pixels.enumerated() where pixel == "#" {
                    marks.append(.box(
                        CGRect(x: at.x - half + Double(column) * Self.pixel, y: at.y + Double(row) * Self.pixel,
                               width: Self.pixel, height: Self.pixel),
                        round: 0,
                        hue: .bright(1),
                        alpha: fade
                    ))
                }
            }
        }
    }
}
