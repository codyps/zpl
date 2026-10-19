import Foundation
import XCTest
@testable import ZPL

final class ZPLTests: XCTestCase {
    private func fixture(_ name: String) throws -> Data {
        let url = Bundle.module.url(forResource: name, withExtension: nil, subdirectory: "Fixtures")!
        return try Data(contentsOf: url)
    }

    func testDirectRustParityForEveryProfileAndOutput() throws {
        let source = try fixture("source.zpl")
        let expectedVersion = String(decoding: try fixture("version.txt"), as: UTF8.self)
        XCTAssertEqual(ZPL.libraryVersion, expectedVersion)
        for profile in Profile.allCases {
            let document = try ZPL.render(source, options: RenderOptions(profile: profile))
            XCTAssertEqual(document.labelCount, 2)
            XCTAssertEqual(try document.warnings.joined(separator: "\n"),
                           String(decoding: try fixture("\(profile)-warnings.txt"), as: UTF8.self))
            XCTAssertEqual(try document.pdf(), try fixture("\(profile)-pages.pdf"))
            for index in 0..<document.labelCount {
                let scene = try document.label(at: index)
                let info = try scene.info
                XCTAssertEqual("\(info.width),\(info.height),\(info.dpi)",
                               String(decoding: try fixture("\(profile)-\(index).info"), as: UTF8.self))
                for (format, ext) in [(OutputFormat.png, "png"), (.svg, "svg"), (.pdf, "pdf"), (.grayscale, "gray")] {
                    XCTAssertEqual(try scene.encode(format), try fixture("\(profile)-\(index).\(ext)"))
                }
            }
        }
    }

    func testSceneAndCopiedBytesOutliveDocument() throws {
        var document: Document? = try ZPL.render("^XA^PW32^LL24^FO0,0^GB8,8,8^FS^XZ",
                                                options: RenderOptions(profile: .specification))
        var scene: Scene? = try document!.label(at: 0)
        document = nil
        let pixels = try scene!.encode(.grayscale)
        scene = nil
        XCTAssertEqual(pixels.count, 32 * 24)
        XCTAssertEqual(Array(pixels.prefix(10)), [0, 0, 0, 0, 0, 0, 0, 0, 255, 255])
        XCTAssertEqual(pixels.last, 255)
    }

    func testBinaryParsingOffsetsAndSyntax() throws {
        let source = Data([0xff, 0, 0x5e, 0x58, 0x41, 0x5e, 0x46, 0x44, 0x41, 0, 0x42,
                           0x5e, 0x46, 0x53, 0x5e, 0x58, 0x5a])
        let parsed = try ZPL.parse(source)
        XCTAssertEqual(parsed.elements.reduce(Data()) { $0 + $1.data }, source)
        var offset = 0
        for element in parsed.elements {
            XCTAssertEqual(element.byteOffset, offset)
            offset += element.data.count
        }
        XCTAssertEqual(parsed.finalSyntax.formatPrefix, 94)
        var syntax = try Syntax()
        syntax.formatPrefix = 33
        let custom = try ZPL.parse("!XA!XZ", syntax: syntax)
        XCTAssertEqual(custom.elements.count, 2)
        XCTAssertEqual(custom.finalSyntax.formatPrefix, 33)
        XCTAssertTrue(try ZPL.parse(Data()).elements.isEmpty)
    }

    func testLimitsAndErrorCopy() throws {
        var limits = try RenderLimits()
        limits.inputBytes = 1
        var saved: ZPLError?
        XCTAssertThrowsError(try ZPL.render("^XA^XZ", limits: limits)) { error in
            saved = error as? ZPLError
            XCTAssertEqual(saved?.status, .renderError)
            XCTAssertFalse(saved?.message.isEmpty ?? true)
        }
        let text = saved?.message
        let document = try ZPL.render("^XA^PW32^LL24^XZ")
        XCTAssertEqual(saved?.message, text)
        XCTAssertThrowsError(try document.label(at: -1))
        XCTAssertThrowsError(try document.label(at: 1))
        var output = try OutputLimits()
        output.pixels = 1
        XCTAssertThrowsError(try document.label(at: 0).encode(.png, limits: output))
        var options = try RenderOptions()
        options.dpi = 0
        XCTAssertThrowsError(try ZPL.render("^XA^XZ", options: options))
        XCTAssertFalse(ZPL.libraryVersion.isEmpty)
    }

    func testNativeSizeLimitsPreserveAllBits() throws {
        var limits = try RenderLimits()
        limits.inputBytes = UInt.max
        XCTAssertEqual(limits.inputBytes, UInt.max)
        var output = try OutputLimits()
        output.pixels = UInt.max
        XCTAssertEqual(output.pixels, UInt.max)
        let document = try ZPL.render("^XA^PW16^LL12^XZ", limits: limits)
        XCTAssertEqual(try document.label(at: 0).encode(.grayscale, limits: output).count, 16 * 12)
        XCTAssertThrowsError(try ZPL.parse("^XA^GFB,2,2,1,")) { error in
            let native = error as? ZPLError
            XCTAssertEqual(native?.status, .parseError)
            XCTAssertNotNil(native?.byteOffset)
        }
    }

    func testTypedCompatibilityAndFonts() throws {
        var options = try RenderOptions(profile: .specification)
        options.compatibility.previewWidthQuantum = 8
        options.compatibility.qrPrinterMaskSelection = true
        options.compatibility.macroPdf417FileId = (1, 2, 3)
        XCTAssertEqual(options.compatibility.previewWidthQuantum, 8)
        XCTAssertTrue(options.compatibility.qrPrinterMaskSelection)
        XCTAssertEqual(options.compatibility.macroPdf417FileId?.2, 3)
        options.compatibility.previewWidthQuantum = nil
        XCTAssertNil(options.compatibility.previewWidthQuantum)
        let fonts = try Fonts()
        XCTAssertThrowsError(try fonts.addTrueType(Data([1, 2, 3]), id: 65)) { error in
            XCTAssertEqual((error as? ZPLError)?.status, .fontError)
        }
        var data = try fixture("probe.ttf")
        try fonts.addTrueType(data, id: 65)
        try fonts.addTrueType(data, filename: "R:PROBE.TTF")
        data.removeAll() // The font collection must own its copied native bytes.
        let document = try ZPL.render("^XA^PW64^LL48^FO4,4^AAN,20,20^FDA^FS^XZ", fonts: fonts)
        XCTAssertEqual(document.labelCount, 1)
        let pixels = try document.label(at: 0).encode(.grayscale)
        XCTAssertTrue(pixels.contains(0))
        XCTAssertTrue(pixels.contains(255))
    }
}
