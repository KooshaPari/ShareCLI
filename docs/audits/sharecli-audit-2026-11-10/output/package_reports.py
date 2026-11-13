"""Generate text PDFs and deterministic-content ZIP inventories. No screen capture."""
from pathlib import Path
from hashlib import sha256
from html import escape
import json, re, zipfile
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
from reportlab.lib.styles import getSampleStyleSheet
from reportlab.lib import colors

root = Path(__file__).resolve().parent
required = [root / "sharecli-audit.md", root / "console-optimization-design.md", root / "dream-loop-phenoregistry.md"]
missing = [str(p) for p in required if not p.exists()]
if missing:
    raise SystemExit("Missing primary reports: " + ", ".join(missing))
styles = getSampleStyleSheet()
styles["BodyText"].fontName = "Helvetica"
styles["BodyText"].fontSize = 9
styles["BodyText"].leading = 13
styles["BodyText"].spaceAfter = 6
styles["Heading1"].textColor = colors.HexColor("#17324d")
pdfdir = root / "pdf"
pdfdir.mkdir(exist_ok=True)

def ascii_text(s):
    return s.replace("\u2014", " - ").replace("\u2013", "-").replace("\u2192", " -> ").replace("\u2019", "'").replace("\u201c", chr(34)).replace("\u201d", chr(34)).encode("ascii", "replace").decode()

def blocks(path):
    out = []
    for line in path.read_text().splitlines():
        if not line.strip():
            out.append(Spacer(1, 4)); continue
        line = ascii_text(line)
        if line.startswith("# "): style, line = "Heading1", line[2:]
        elif line.startswith("## "): style, line = "Heading2", line[3:]
        elif line.startswith("### "): style, line = "Heading3", line[4:]
        else: style = "BodyText"
        # Long code paths and URLs must wrap without clipping.
        safe = escape(line)
        safe = safe
        out.append(Paragraph(safe, styles[style]))
    return out

def footer(canvas, doc):
    canvas.saveState();canvas.setFont("Helvetica", 8)
    canvas.drawString(42, 25, "ShareCLI audit | derived report | 2026-10-14")
    canvas.drawRightString(570, 25, str(doc.page));canvas.restoreState()

combined = []
for i, p in enumerate(required):
    if i: combined.append(PageBreak())
    combined.extend(blocks(p))
SimpleDocTemplate(str(pdfdir / "sharecli-comprehensive-report.pdf"), rightMargin=42, leftMargin=42, topMargin=42, bottomMargin=42).build(combined, onFirstPage=footer, onLaterPages=footer)

# Separate curated report bundle from explicitly requested private chat exports.
reports = sorted(p for p in root.rglob("*") if p.is_file() and p.suffix in {".md", ".pdf"} and p.name != "selected-chat-excerpts.md" and "repo" not in p.relative_to(root).parts)
chats = sorted(root.glob("chat-*.json"))
chats = [p for p in chats if isinstance(json.loads(p.read_text()), dict)]
for name, files in [("sharecli-report-bundle.zip", reports), ("sharecli-selected-chat-exports.zip", chats + [root / "selected-chat-excerpts.md", root / "source-register.md", root / "operator-brief.md"])]:
    manifest = {str(p.relative_to(root)): {"bytes": p.stat().st_size, "sha256": sha256(p.read_bytes()).hexdigest()} for p in files}
    with zipfile.ZipFile(root / name, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for p in files:z.write(p, p.relative_to(root))
        z.writestr("MANIFEST.json", json.dumps(manifest, indent=2))
    with zipfile.ZipFile(root / name) as z:
        assert z.testzip() is None
        for rel, item in manifest.items():assert sha256(z.read(rel)).hexdigest() == item["sha256"]
    print(name, len(files), "files; CRC and SHA256 verified")
artifacts = sorted(p for p in root.rglob("*") if p.is_file() and p.name != "SHA256SUMS.json")
(root / "SHA256SUMS.json").write_text(json.dumps({str(p.relative_to(root)): sha256(p.read_bytes()).hexdigest() for p in artifacts}, indent=2))
print("PDF:", pdfdir / "sharecli-comprehensive-report.pdf")
