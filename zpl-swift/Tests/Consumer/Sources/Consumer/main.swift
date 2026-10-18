import Foundation
import ZPL

let document = try ZPL.render("^XA^PW32^LL24^FO0,0^GB8,8,8^FS^XZ",
                              options: RenderOptions(profile: .specification))
precondition(document.labelCount == 1)
let scene = try document.label(at: 0)
let pixels = try scene.encode(.grayscale)
precondition(pixels.count == 32 * 24 && pixels.first == 0 && pixels.last == 255)
let pdf = try document.pdf()
precondition(pdf.starts(with: Data("%PDF-".utf8)))
print("ZPL \(ZPL.libraryVersion): independent consumer passed")
