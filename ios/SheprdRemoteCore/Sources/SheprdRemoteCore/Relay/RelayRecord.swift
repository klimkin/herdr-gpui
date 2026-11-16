import CryptoKit
import Foundation

/// How this phone reaches one computer through its relays: what the
/// computer said over a path the phone already trusted (`GET /api/relay`),
/// and the keys the phone made for it and sent (`POST /api/relay`). Kept
/// in the computer's Keychain record, as its device key is.
public struct RelayRecord: Codable, Hashable, Sendable {
    /// The relays the computer listens on.
    public var relays: [RelayEndpoint]
    /// The computer's route: the SHA-256 of its relay key.
    public var route: Data
    /// The computer's tunnel identity, pinned: X25519 then ML-KEM-768.
    public var identity: Data
    /// This phone's dial key for the computer's relays (P-256, raw).
    public var dialKey: Data
    /// This phone's tunnel static key for the computer (X25519, raw).
    public var tunnelKey: Data
    /// What the computer's owner calls each relay, keyed by the endpoint's
    /// `description`; refreshed whenever the computer says, without new keys.
    public var labels: [String: RelayLabel]

    public init(
        relays: [RelayEndpoint], route: Data, identity: Data, dialKey: Data, tunnelKey: Data,
        labels: [String: RelayLabel] = [:]
    ) {
        self.relays = relays
        self.route = route
        self.identity = identity
        self.dialKey = dialKey
        self.tunnelKey = tunnelKey
        self.labels = labels
    }

    enum CodingKeys: String, CodingKey { case relays, route, identity, dialKey, tunnelKey, labels }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        relays = try c.decode([RelayEndpoint].self, forKey: .relays)
        route = try c.decode(Data.self, forKey: .route)
        identity = try c.decode(Data.self, forKey: .identity)
        dialKey = try c.decode(Data.self, forKey: .dialKey)
        tunnelKey = try c.decode(Data.self, forKey: .tunnelKey)
        // Records saved before relays had labels have none.
        labels = try c.decodeIfPresent([String: RelayLabel].self, forKey: .labels) ?? [:]
    }

    /// A record for what `info` says, with fresh keys for this phone; nil
    /// when `info` is not a relay answer.
    public static func make(from info: API.RelayInfo) -> RelayRecord? {
        guard let route = Base64URL.decode(info.route), route.count == 32,
              let identity = Base64URL.decode(info.identity),
              (try? Tunnel.Identity(bytes: identity)) != nil
        else { return nil }
        let relays = info.relays.compactMap(RelayEndpoint.init)
        guard !relays.isEmpty else { return nil }
        return RelayRecord(
            relays: relays,
            route: route,
            identity: identity,
            dialKey: P256.Signing.PrivateKey().rawRepresentation,
            tunnelKey: Curve25519.KeyAgreement.PrivateKey().rawRepresentation,
            labels: labels(from: info)
        )
    }

    /// What `POST /api/relay` sends: the public halves, base64url.
    public var enrollment: API.RelayKeys? {
        guard let dial = try? P256.Signing.PrivateKey(rawRepresentation: dialKey),
              let tunnel = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        else { return nil }
        return API.RelayKeys(
            dial: Base64URL.encode(dial.publicKey.compressedRepresentation),
            tunnel: Base64URL.encode(tunnel.publicKey.rawRepresentation)
        )
    }

    /// This record with the labels `info` gives, or nil when they are the
    /// ones it has: labels change without new keys.
    public func relabeled(_ info: API.RelayInfo) -> RelayRecord? {
        let fresh = Self.labels(from: info)
        guard fresh != labels else { return nil }
        var record = self
        record.labels = fresh
        return record
    }

    /// Whether `info` still names the same computer and relays, so the
    /// keys already sent hold. Labels do not count (`relabeled`).
    public func matches(_ info: API.RelayInfo) -> Bool {
        Base64URL.decode(info.route) == route
            && Base64URL.decode(info.identity) == identity
            && Set(info.relays.compactMap(RelayEndpoint.init)) == Set(relays)
    }

    /// Opens the pairing session a nearby offer allowed, through the first
    /// relay that answers: the one-dial key and the tunnel's pairing PSK
    /// both come from the offer's token, `secret`.
    public func openPairing(secret: Data, session: URLSession = .shared) async throws -> RelaySession {
        let dial = RelayProof.pairingDialKey(secret: secret)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        var last: Error = RelaySession.Failure.closed
        for relay in relays {
            do {
                return try await RelaySession.open(
                    endpoint: relay, route: route, dialKey: dial, tunnel: .pairing(local: tunnel, listener: listener, secret: secret),
                    session: session
                )
            } catch {
                last = error
            }
        }
        throw last
    }

    /// The relays to dial the computer on, in order: the ones it named,
    /// then those of `saved` it did not. Its route and pinned identity
    /// are the same on every relay, so a saved relay it does not listen on
    /// only answers that it is offline.
    public func candidates(saved: [RelayEndpoint]) -> [RelayEndpoint] {
        relays + saved.filter { !relays.contains($0) }
    }

    /// Opens a session to the computer through the first relay that
    /// answers, of its own and then of `saved`. A relay that says nothing
    /// holds the next one back for `RelayRace.stagger` only, so a computer
    /// that moved off a dead relay is still reached through a saved one.
    public func open(saved: [RelayEndpoint] = [], session: URLSession = .shared) async throws -> RelaySession {
        let dial = try P256.Signing.PrivateKey(rawRepresentation: dialKey)
        let tunnel = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: tunnelKey)
        let listener = try Tunnel.Identity(bytes: identity)
        let route = route
        return try await RelayRace.first(
            of: candidates(saved: saved),
            none: RelaySession.Failure.closed,
            open: { relay in
                try await RelaySession.open(
                    endpoint: relay, route: route, dialKey: dial,
                    tunnelKey: tunnel, listener: listener, session: session
                )
            },
            discard: { await $0.close() }
        )
    }
}

extension API {
    /// `GET /api/relay`.
    public struct RelayInfo: Codable, Hashable, Sendable {
        public let identity: String
        public let relays: [String]
        public let route: String
        /// What the owner calls each relay, by endpoint; nil from a daemon
        /// without labels, or none to give.
        public let labels: [String: RelayLabel]?

        public init(identity: String, relays: [String], route: String, labels: [String: RelayLabel]? = nil) {
            self.identity = identity
            self.relays = relays
            self.route = route
            self.labels = labels
        }
    }

    /// `POST /api/relay`.
    public struct RelayKeys: Codable, Equatable, Sendable {
        public let dial: String
        public let tunnel: String
    }
}
