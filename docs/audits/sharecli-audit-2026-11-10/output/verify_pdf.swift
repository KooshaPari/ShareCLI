import Foundation
import PDFKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
guard let document = PDFDocument(url: url) else { fatalError("Cannot parse PDF") }
let text = document.string ?? ""
let result: [String: Any] = ["pages": document.pageCount, "characters": text.count, "hasShareCLI": text.contains("ShareCLI"), "hasWSL": text.contains("WSL"), "verification": "PDFKit text extraction; no rendering or screen capture"]
let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
print(String(data: data, encoding: .utf8)!)
if document.pageCount == 0 || text.count < 10000 { exit(1) }
