import ApplicationServices
import CryptoKit
import Darwin
import Foundation

/// The byte protocol the host speaks to this agent over the virtual machine's socket device.
enum Wire {
    static let port: UInt32 = 7273
    static let agentVersion = 1
    static let hostAddress: UInt32 = 2
    static let anyAddress: UInt32 = UInt32.max
    static let frameLimit = 65536
    static let requestLimit = 1 << 20
    static let stdout: UInt8 = 1
    static let stderr: UInt8 = 2
    static let exit: UInt8 = 3
    static let failure: UInt8 = 4
    static let nonceSize = 32
    static let proofContext = Data("agent-desktop-guest-v1".utf8)
}

enum GuestPaths {
    static var share: String { Transport.share }
    static let binaries = ["agent-desktop", "agent-desktop-macos-helper"]
    static var bin: String { Transport.home + "/.local/bin" }
    static var command: String { bin + "/agent-desktop" }
    static var installedBuild: String { bin + "/.agent-desktop-build-id" }
}

struct AgentError: Error {
    let message: String
}

struct Request: Decodable {
    let op: String
    let build: String?
    let args: [String]?
}

/// Layout of `struct sockaddr_vm` in <sys/vsock.h>.
struct SocketAddress {
    var length = UInt8(MemoryLayout<SocketAddress>.size)
    var family = sa_family_t(AF_VSOCK)
    var reserved: UInt16 = 0
    var port: UInt32 = 0
    var address: UInt32 = 0
}

final class Channel: @unchecked Sendable {
    private let descriptor: Int32
    private let lock = NSLock()

    init(_ descriptor: Int32) {
        self.descriptor = descriptor
    }

    func read(_ count: Int) -> Data? {
        var bytes = [UInt8](repeating: 0, count: count)
        var filled = 0
        while filled < count {
            let got = bytes.withUnsafeMutableBytes { Darwin.read(descriptor, $0.baseAddress! + filled, count - filled) }
            if got < 0 && errno == EINTR { continue }
            guard got > 0 else { return nil }
            filled += got
        }
        return Data(bytes)
    }

    func send(_ kind: UInt8, _ payload: Data) {
        lock.withLock {
            var offset = 0
            repeat {
                let chunk = payload.subdata(in: offset..<min(payload.count, offset + Wire.frameLimit))
                var frame = Data([kind])
                withUnsafeBytes(of: UInt32(chunk.count).bigEndian) { frame.append(contentsOf: $0) }
                frame.append(chunk)
                guard write(frame) else { return }
                offset += chunk.count
            } while offset < payload.count
        }
    }

    func fail(_ message: String) {
        send(Wire.failure, Data(message.utf8))
    }

    /// Returns once the host closes its end, which is how a cancelled command reaches the guest.
    func waitForHangup() {
        var scratch = [UInt8](repeating: 0, count: 512)
        while true {
            let got = Darwin.read(descriptor, &scratch, scratch.count)
            if got < 0 && errno == EINTR { continue }
            if got <= 0 { return }
        }
    }

    func prove(_ proof: Data) -> Bool {
        lock.withLock { write(proof) }
    }

    func hangUp() {
        shutdown(descriptor, SHUT_RDWR)
    }

    func close() {
        Darwin.close(descriptor)
    }

    private func write(_ data: Data) -> Bool {
        var sent = 0
        while sent < data.count {
            let wrote = data.withUnsafeBytes { Darwin.write(descriptor, $0.baseAddress! + sent, data.count - sent) }
            if wrote < 0 && errno == EINTR { continue }
            guard wrote > 0 else { return false }
            sent += wrote
        }
        return true
    }
}

/// Installs the host's agent-desktop build from the shared folder when the guest holds a different one. The host
/// shares every build under its own id, because a guest never sees a shared file change after its first read.
enum BuildSync {
    private static let lock = NSLock()

    static func apply(_ build: String) throws {
        try lock.withLock {
            guard build.count == 40, build.allSatisfy({ $0.isHexDigit }) else {
                throw AgentError(message: "The host did not name the agent-desktop build to run.")
            }
            let files = FileManager.default
            let installed = GuestPaths.binaries.allSatisfy { files.isExecutableFile(atPath: GuestPaths.bin + "/" + $0) }
            if installed, let current = files.contents(atPath: GuestPaths.installedBuild), current == Data(build.utf8) { return }
            let source = GuestPaths.share + "/builds/" + build + "/"
            do {
                try files.createDirectory(atPath: GuestPaths.bin, withIntermediateDirectories: true)
                for name in GuestPaths.binaries {
                    let staged = GuestPaths.bin + "/" + name + ".new"
                    try? files.removeItem(atPath: staged)
                    try files.copyItem(atPath: source + name, toPath: staged)
                    try files.setAttributes([.posixPermissions: 0o755], ofItemAtPath: staged)
                    guard rename(staged, GuestPaths.bin + "/" + name) == 0 else { throw AgentError(message: "") }
                }
                try Data(build.utf8).write(to: URL(fileURLWithPath: GuestPaths.installedBuild), options: .atomic)
            } catch {
                throw AgentError(message: "agent-desktop could not be copied from the sandbox's shared folder into the guest.")
            }
        }
    }
}

enum Command {
    static func run(_ request: Request, over channel: Channel) {
        do {
            try BuildSync.apply(request.build ?? "")
        } catch {
            channel.fail((error as? AgentError)?.message ?? "agent-desktop could not be prepared in the guest.")
            return
        }
        let process = Process()
        let output = Pipe()
        let errors = Pipe()
        process.executableURL = URL(fileURLWithPath: GuestPaths.command)
        process.arguments = request.args ?? []
        process.currentDirectoryURL = URL(fileURLWithPath: Transport.home, isDirectory: true)
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = errors
        do {
            try process.run()
        } catch {
            channel.fail("agent-desktop could not be started in the guest.")
            return
        }
        let streams = DispatchGroup()
        forward(output, as: Wire.stdout, to: channel, in: streams)
        forward(errors, as: Wire.stderr, to: channel, in: streams)
        let watcher = DispatchSemaphore(value: 0)
        Thread.detachNewThread {
            channel.waitForHangup()
            if process.isRunning { process.terminate() }
            watcher.signal()
        }
        streams.wait()
        process.waitUntilExit()
        let status = process.terminationReason == .exit ? process.terminationStatus : 128 + process.terminationStatus
        var reply = Data()
        withUnsafeBytes(of: UInt32(bitPattern: status).bigEndian) { reply.append(contentsOf: $0) }
        channel.send(Wire.exit, reply)
        channel.hangUp()
        watcher.wait()
    }

    private static func forward(_ pipe: Pipe, as kind: UInt8, to channel: Channel, in group: DispatchGroup) {
        group.enter()
        Thread.detachNewThread {
            while true {
                let data = pipe.fileHandleForReading.availableData
                if data.isEmpty { break }
                channel.send(kind, data)
            }
            group.leave()
        }
    }
}

enum Permissions {
    static let probeArgument = "accessibility"

    /// A running process keeps the answer macOS gave it at launch, so a fresh copy of the agent asks each time.
    static var accessibility: Bool {
        let probe = Process()
        probe.executableURL = URL(fileURLWithPath: Bundle.main.executablePath ?? CommandLine.arguments[0])
        probe.arguments = [probeArgument]
        probe.standardInput = FileHandle.nullDevice
        probe.standardOutput = FileHandle.nullDevice
        probe.standardError = FileHandle.nullDevice
        guard (try? probe.run()) != nil else { return false }
        probe.waitUntilExit()
        return probe.terminationReason == .exit && probe.terminationStatus == 0
    }

    /// Puts this agent in the Accessibility and Screen Recording lists so a person can allow it.
    static func request() {
        if !AXIsProcessTrusted() {
            let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
            _ = AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary)
        }
        if !CGPreflightScreenCaptureAccess() {
            _ = CGRequestScreenCaptureAccess()
        }
    }
}

/// Where the agent listens and what it reads from its surroundings. A build with GUEST_AGENT_TEST serves a
/// socket file and takes its paths and peer address from the environment, so tests run the protocol without a
/// virtual machine. A release build contains none of that.
enum Transport {
#if GUEST_AGENT_TEST
    static let environment = ProcessInfo.processInfo.environment
    static var share: String { environment["GUEST_TEST_SHARE"] ?? "" }
    static var home: String { environment["GUEST_TEST_HOME"] ?? "" }

    static func requestPermissions() {}

    static func peer(of descriptor: Int32) -> SocketAddress? {
        SocketAddress(address: UInt32(environment["GUEST_TEST_PEER"] ?? "") ?? Wire.hostAddress)
    }

    static func listen() -> Int32? {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0, let path = environment["GUEST_TEST_SOCKET"] else { return nil }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: path.utf8) }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, Darwin.listen(descriptor, 16) == 0 else { return nil }
        return descriptor
    }
#else
    static let share = "/Volumes/My Shared Files"
    static var home: String { NSHomeDirectory() }

    static func requestPermissions() {
        Permissions.request()
    }

    static func peer(of descriptor: Int32) -> SocketAddress? {
        var address = SocketAddress()
        var size = socklen_t(MemoryLayout<SocketAddress>.size)
        let status = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getpeername(descriptor, $0, &size) }
        }
        return status == 0 ? address : nil
    }

    static func listen() -> Int32? {
        let descriptor = socket(AF_VSOCK, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return nil }
        var address = SocketAddress(port: Wire.port, address: Wire.anyAddress)
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<SocketAddress>.size))
            }
        }
        guard bound == 0, Darwin.listen(descriptor, 16) == 0 else {
            close(descriptor)
            return nil
        }
        return descriptor
    }
#endif
}

/// Any account in the guest can listen on the agent's port. The agent signs the host's nonce with a key that
/// only its own account can read, and the host sends work only to the key it recorded when setup completed.
enum Identity {
    static let key: Curve25519.Signing.PrivateKey? = load()

    static func proof(for nonce: Data) -> Data? {
        guard let key, let signature = try? key.signature(for: Wire.proofContext + nonce) else { return nil }
        return key.publicKey.rawRepresentation + signature
    }

    private static func load() -> Curve25519.Signing.PrivateKey? {
        let directory = Transport.home + "/Library/Application Support/agent-desktop-guest"
        let path = directory + "/key"
        let files = FileManager.default
        if let stored = files.contents(atPath: path) {
            return try? Curve25519.Signing.PrivateKey(rawRepresentation: stored)
        }
        let key = Curve25519.Signing.PrivateKey()
        do {
            try files.createDirectory(atPath: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        } catch {
            return nil
        }
        let saved = files.createFile(atPath: path, contents: key.rawRepresentation, attributes: [.posixPermissions: 0o600])
        return saved ? key : nil
    }
}

enum Agent {
    static func serve(_ channel: Channel) {
        defer { channel.close() }
        guard let nonce = channel.read(Wire.nonceSize), let proof = Identity.proof(for: nonce), channel.prove(proof) else { return }
        guard let header = channel.read(4) else { return }
        let length = header.reduce(0) { ($0 << 8) | Int($1) }
        guard length > 0, length <= Wire.requestLimit, let body = channel.read(length),
              let request = try? JSONDecoder().decode(Request.self, from: body) else {
            channel.fail("The guest agent could not read the request.")
            return
        }
        switch request.op {
        case "status":
            let state = "{\"accessibility\":\(Permissions.accessibility),\"agent\":\(Wire.agentVersion)}\n"
            channel.send(Wire.stdout, Data(state.utf8))
            channel.send(Wire.exit, Data([0, 0, 0, 0]))
        case "exec":
            Command.run(request, over: channel)
        default:
            channel.fail("The guest agent does not know the operation \(request.op).")
        }
    }

    /// Only the host that runs this virtual machine may give the agent work.
    static func isHost(_ peer: SocketAddress?) -> Bool {
        guard let peer else { return false }
        return peer.family == sa_family_t(AF_VSOCK) && peer.address == Wire.hostAddress
    }

    static func run() -> Int32 {
        if CommandLine.arguments.dropFirst().first == Permissions.probeArgument {
            return AXIsProcessTrusted() ? 0 : 1
        }
        signal(SIGPIPE, SIG_IGN)
        guard Identity.key != nil else {
            FileHandle.standardError.write(Data("agent-desktop-guest: the agent's key cannot be read or created\n".utf8))
            return 1
        }
        guard let listener = Transport.listen() else {
            FileHandle.standardError.write(Data("agent-desktop-guest: the virtual machine socket is not available: \(String(cString: strerror(errno)))\n".utf8))
            return 1
        }
        Transport.requestPermissions()
        while true {
            let descriptor = accept(listener, nil, nil)
            if descriptor < 0 {
                if errno == EINTR { continue }
                return 1
            }
            guard isHost(Transport.peer(of: descriptor)) else {
                close(descriptor)
                continue
            }
            let channel = Channel(descriptor)
            Thread.detachNewThread { serve(channel) }
        }
    }
}

exit(Agent.run())
