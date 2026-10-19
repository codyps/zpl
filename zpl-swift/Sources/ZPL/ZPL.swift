import CZpl
import Foundation

public enum Profile: UInt32, CaseIterable, Sendable {
    case zd621 = 0, specification = 1, zq610Plus = 2, gx430t = 3
}

public enum OutputFormat: UInt32, CaseIterable, Sendable {
    case png = 0, svg = 1, pdf = 2, grayscale = 3
}

public enum Status: Int32, Sendable {
    case abiMismatch = -1
    case invalidArgument = 1, parseError = 2, renderError = 3
    case outputError = 4, panic = 5, fontError = 6
}

public struct ZPLError: Error, CustomStringConvertible, Sendable {
    public let status: Status
    public let byteOffset: Int?
    public let message: String
    public var description: String { message }
}

internal func checkABI() throws {
    guard zpl_abi_version() == UInt32(ZPL_ABI_VERSION) else {
        throw ZPLError(status: .abiMismatch, byteOffset: nil, message: "ZPL headers and library ABI differ")
    }
}

private func copyBytes(_ bytes: ZplBytes) -> Data {
    guard bytes.len > 0 else { return Data() }
    return Data(bytes: bytes.data!, count: Int(bytes.len))
}

internal func check(_ status: Int32) throws {
    guard status != ZPL_OK else { return }
    // Copy thread-local error storage before any subsequent fallible C call.
    let error = zpl_last_error()
    throw ZPLError(status: Status(rawValue: status) ?? .panic,
                   byteOffset: error.offset == ~0 ? nil : Int(error.offset),
                   message: String(decoding: copyBytes(error.message), as: UTF8.self))
}

private func withOptional<T, R>(_ value: T?, _ body: (UnsafePointer<T>?) throws -> R) rethrows -> R {
    guard var value = value else { return try body(nil) }
    return try withUnsafePointer(to: &value) { try body($0) }
}

private func encoded(_ operation: (UnsafeMutablePointer<OpaquePointer?>) -> Int32) throws -> Data {
    var buffer: OpaquePointer?
    try check(operation(&buffer))
    defer { zpl_buffer_free(buffer) }
    return copyBytes(zpl_buffer_bytes(buffer))
}

public struct RenderOptions {
    internal var raw: ZplOptions

    public init(profile: Profile = .zd621) throws {
        try checkABI()
        var value = ZplOptions()
        try check(zpl_options_init(profile.rawValue, &value))
        raw = value
    }

    public var width: UInt32 {
        get { raw.width }
        set { raw.width = newValue }
    }
    public var height: UInt32 {
        get { raw.height }
        set { raw.height = newValue }
    }
    public var dpi: UInt32 {
        get { raw.dpi }
        set { raw.dpi = newValue }
    }
    public var compatibility: Compatibility {
        get { Compatibility(raw: raw.compatibility) }
        set { raw.compatibility = newValue.raw }
    }
}

/// Synchronous, local parsing and rendering. Data input preserves binary graphics,
/// embedded NULs, and legacy character encodings; String input is encoded as UTF-8.
public enum ZPL {
    public static var libraryVersion: String {
        String(decoding: copyBytes(zpl_library_version()), as: UTF8.self)
    }

    public static func render(_ source: String, options: RenderOptions? = nil,
                              limits: RenderLimits? = nil, fonts: Fonts? = nil) throws -> Document {
        try render(Data(source.utf8), options: options, limits: limits, fonts: fonts)
    }

    public static func render(_ source: Data, options: RenderOptions? = nil,
                              limits: RenderLimits? = nil, fonts: Fonts? = nil) throws -> Document {
        try checkABI()
        return try source.withUnsafeBytes { bytes in
            try withOptional(options?.raw) { options in
                try withOptional(limits?.raw) { limits in
                    var document: OpaquePointer?
                    // Fonts serializes mutations and rendering; native scenes never retain it.
                    let operation = {
                        try check(zpl_render_with_fonts(bytes.bindMemory(to: UInt8.self).baseAddress,
                                                        .init(bytes.count), options, fonts?.handle, limits, &document))
                        return Document(handle: document!)
                    }
                    if let fonts = fonts { return try fonts.locked(operation) }
                    return try operation()
                }
            }
        }
    }

    public static func parse(_ source: String, syntax: Syntax? = nil) throws -> ParseResult {
        try parse(Data(source.utf8), syntax: syntax)
    }

    /// Lossless framing only. Bound input size before calling; parsing does not
    /// validate commands or authorize forwarding them to a printer.
    public static func parse(_ source: Data, syntax: Syntax? = nil) throws -> ParseResult {
        try checkABI()
        return try source.withUnsafeBytes { bytes in
            try withOptional(syntax?.raw) { syntax in
                var parsed: OpaquePointer?
                try check(zpl_parse(bytes.bindMemory(to: UInt8.self).baseAddress, .init(bytes.count), syntax, &parsed))
                defer { zpl_parsed_free(parsed) }
                var elements: [ParsedElement] = []
                for index in 0..<Int(zpl_parsed_count(parsed)) {
                    var element = ZplElement()
                    try check(zpl_parsed_element(parsed, .init(index), &element))
                    elements.append(ParsedElement(kind: ElementKind(rawValue: element.kind)!,
                                                   byteOffset: Int(element.offset), data: copyBytes(element.data)))
                }
                var finalSyntax = ZplSyntax()
                try check(zpl_parsed_syntax(parsed, &finalSyntax))
                return ParseResult(elements: elements, finalSyntax: Syntax(raw: finalSyntax))
            }
        }
    }
}

/// Immutable native document, released automatically when the last reference dies.
public final class Document {
    private let handle: OpaquePointer
    internal init(handle: OpaquePointer) { self.handle = handle }
    deinit { zpl_document_free(handle) }

    public var labelCount: Int { Int(zpl_document_label_count(handle)) }
    public var warnings: [String] {
        get throws {
            try (0..<Int(zpl_document_warning_count(handle))).map { index in
                var bytes = ZplBytes()
                try check(zpl_document_warning(handle, .init(index), &bytes))
                return String(decoding: copyBytes(bytes), as: UTF8.self)
            }
        }
    }

    /// The label owns a scene snapshot and may outlive this document.
    public func label(at index: Int) throws -> Scene {
        guard index >= 0 else {
            throw ZPLError(status: .invalidArgument, byteOffset: nil, message: "Label index must be nonnegative")
        }
        var scene: OpaquePointer?
        try check(zpl_document_label(handle, .init(index), &scene))
        return Scene(handle: scene!)
    }

    public func pdf(limits: OutputLimits? = nil) throws -> Data {
        try withOptional(limits?.raw) { limits in
            try encoded { zpl_document_pdf(handle, limits, $0) }
        }
    }
}

public struct SceneInfo: Equatable, Sendable {
    public let width: UInt32
    public let height: UInt32
    public let dpi: UInt32
}

public final class Scene {
    private let handle: OpaquePointer
    internal init(handle: OpaquePointer) { self.handle = handle }
    deinit { zpl_scene_free(handle) }

    public var info: SceneInfo {
        get throws {
            var value = ZplSceneInfo()
            try check(zpl_scene_info(handle, &value))
            return SceneInfo(width: value.width, height: value.height, dpi: value.dpi)
        }
    }

    /// Grayscale output is row-major, one byte per dot, 0 black / 255 white.
    /// Returned Data owns its bytes independently of all native handles.
    public func encode(_ format: OutputFormat, limits: OutputLimits? = nil) throws -> Data {
        try withOptional(limits?.raw) { limits in
            try encoded { zpl_scene_encode(handle, format.rawValue, limits, $0) }
        }
    }
}

public enum ElementKind: UInt32, Sendable {
    case beforeFirstCommand = 0, formatCommand = 1, controlCommand = 2, controlCharacter = 3
}

public struct ParsedElement: Equatable, Sendable {
    public let kind: ElementKind
    public let byteOffset: Int
    public let data: Data
}

public struct ParseResult {
    public let elements: [ParsedElement]
    public let finalSyntax: Syntax
}

public enum Hinting: UInt32, Sendable { case none = 0, native = 1 }

/// A font collection owns copied TrueType data. Mutation and rendering are
/// serialized. Font IDs are Unicode scalar values, e.g. 65 for "A".
public final class Fonts {
    fileprivate let handle: OpaquePointer
    private let lock = NSLock()
    public init() throws {
        try checkABI()
        var value: OpaquePointer?
        try check(zpl_fonts_new(&value))
        handle = value!
    }
    deinit { zpl_fonts_free(handle) }

    fileprivate func locked<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }

    public func addTrueType(_ data: Data, id: UInt32, hinting: Hinting = .native) throws {
        try locked {
            try data.withUnsafeBytes { bytes in
                try check(zpl_fonts_insert_truetype(handle, id, bytes.bindMemory(to: UInt8.self).baseAddress,
                                                    .init(bytes.count), hinting.rawValue))
            }
        }
    }

    public func addTrueType(_ data: Data, filename: String, hinting: Hinting = .native) throws {
        try locked {
            try Data(filename.utf8).withUnsafeBytes { name in
                try data.withUnsafeBytes { bytes in
                    try check(zpl_fonts_insert_named_truetype(handle, name.bindMemory(to: UInt8.self).baseAddress,
                                .init(name.count), bytes.bindMemory(to: UInt8.self).baseAddress, .init(bytes.count), hinting.rawValue))
                }
            }
        }
    }
}
