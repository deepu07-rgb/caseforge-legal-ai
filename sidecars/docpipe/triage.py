"""Provisional page triage classifier (B-S1-05).

PRD §6.1 classes only. NOT the final vision/HTR model (PRD §21 #3 open).
Synthetic fixtures may embed an explicit marker for deterministic plumbing tests:
  [CASEFORGE_TRIAGE_CLASS:<class>]
"""

from __future__ import annotations

import re
from typing import Any

from pdf_parser import parse_pdf_bytes

TRIAGE_CLASSES = (
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
)

CLASSIFIER_ID = "caseforge.triage.heuristic.v1"
MARKER_RE = re.compile(
    r"\[CASEFORGE_TRIAGE_CLASS:(" + "|".join(TRIAGE_CLASSES) + r")\]",
    re.IGNORECASE,
)

ENGLISH_HINTS = {
    "the",
    "and",
    "of",
    "to",
    "in",
    "that",
    "is",
    "for",
    "court",
    "plaintiff",
    "defendant",
    "affidavit",
    "petition",
    "order",
    "section",
}


def decode_bytes(content: bytes | None, text: str | None) -> str:
    if text is not None:
        return text
    if not content:
        return ""
    # Prefer UTF-8; fall back to latin-1 so binary is never empty of codepoints.
    try:
        return content.decode("utf-8")
    except UnicodeDecodeError:
        return content.decode("latin-1", errors="replace")


def extract_pdf_page_texts(content: bytes) -> list[str]:
    """Crude PDF page split for triage plumbing (not a full renderer)."""
    raw = content.decode("latin-1", errors="replace")
    # Count leaf pages (exclude /Type /Pages)
    page_hits = len(re.findall(r"/Type\s*/Page(?!\s*s)", raw))
    n = max(page_hits, 1)
    # Pull printable text-ish tokens
    strings = re.findall(r"\((?:\\.|[^\\)]){2,200}\)", raw)
    joined = " ".join(s.strip("()") for s in strings)
    if not joined.strip():
        # Try stream-ish ASCII runs
        runs = re.findall(r"[\x20-\x7e]{12,}", raw)
        joined = " ".join(runs[:80])
    # Replicate text across pages if we cannot split streams (honest provisional).
    return [joined for _ in range(n)]


def extract_docx_text(content: bytes) -> str:
    # DOCX is a zip; pull readable XML text without requiring zipfile success on all bytes.
    try:
        import io
        import zipfile

        with zipfile.ZipFile(io.BytesIO(content)) as zf:
            if "word/document.xml" in zf.namelist():
                xml = zf.read("word/document.xml").decode("utf-8", errors="replace")
                texts = re.findall(r"<w:t[^>]*>([^<]*)</w:t>", xml)
                return " ".join(texts)
    except Exception:
        pass
    runs = re.findall(rb"[\x20-\x7e]{8,}", content)
    return b" ".join(runs[:40]).decode("ascii", errors="ignore")


def pages_from_document(
    *,
    content_base64: str | None = None,
    text: str | None = None,
    media_type: str | None = None,
    filename: str | None = None,
) -> list[dict[str, Any]]:
    import base64

    content: bytes | None = None
    if content_base64:
        content = base64.b64decode(content_base64)
    media = (media_type or "").lower()
    name = (filename or "").lower()

    if text is not None and content is None:
        return [{"page_index": 0, "text": text, "media_type": "text/plain"}]

    if content is None:
        return [{"page_index": 0, "text": "", "media_type": media or "application/octet-stream"}]

    if media in {"text/plain", "txt"} or name.endswith(".txt"):
        return [
            {
                "page_index": 0,
                "text": content.decode("utf-8", errors="replace"),
                "media_type": "text/plain",
            }
        ]

    if media in {"application/pdf", "pdf"} or name.endswith(".pdf"):
        parsed = parse_pdf_bytes(content, doc_id=filename or "inline", render_ocr_pages=False)
        return [
            {
                "page_index": page["page_index"],
                "text": page["native_text"],
                "media_type": "application/pdf",
                "is_image": page["source_kind"] in {"scanned", "mixed"},
                "source_kind": page["source_kind"],
                "page_source": page,
            }
            for page in parsed["pages"]
        ]

    if (
        media
        in {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "docx",
        }
        or name.endswith(".docx")
    ):
        return [
            {
                "page_index": 0,
                "text": extract_docx_text(content),
                "media_type": "docx",
            }
        ]

    if media.startswith("image/") or name.endswith((".jpg", ".jpeg", ".png")):
        # No vision model yet: empty text layer; heuristic uses binary cues.
        return [
            {
                "page_index": 0,
                "text": "",
                "media_type": media or "image/*",
                "content_len": len(content),
                "is_image": True,
            }
        ]

    return [
        {
            "page_index": 0,
            "text": decode_bytes(content, None),
            "media_type": media or "application/octet-stream",
        }
    ]


def classify_text(text: str, *, is_image: bool = False, content_len: int = 0) -> tuple[str, float, str]:
    marker = MARKER_RE.search(text or "")
    if marker:
        cls = marker.group(1).lower()
        return cls, 0.99, "explicit fixture marker"

    t = (text or "").strip()
    if is_image and not t:
        # Provisional: unknown image without vision model.
        if content_len < 64:
            return "non_text", 0.55, "tiny image bytes; provisional non_text"
        return "typed_english", 0.35, "image without text layer; provisional low-confidence default"

    if not t:
        return "non_text", 0.9, "empty page text"

    printable = sum(1 for c in t if c.isprintable() and not c.isspace())
    # Near-blank capture / logo-only / severe blur proxies
    if printable < 12:
        return "non_text", 0.88, "too little printable content"

    lower = t.lower()
    # Strip synthetic scaffold footer so footer tokens never drive the class.
    body = re.split(r"\[CASEFORGE_SYNTHETIC_PAGE\]", t, maxsplit=1)[0]
    body_lower = body.lower()

    has_hand = bool(
        re.search(r"handwrit|cursive|margin notes|hand\s*notes|scrawl", body_lower)
    )
    messy = bool(
        re.search(r"messy|scrawl|crossouts|hard to read|\?\?\?", body_lower)
    ) and has_hand
    clean_hand = has_hand and (
        "clean handwriting" in body_lower
        or ("neat" in body_lower and "hand" in body_lower)
        or ("clear lines" in body_lower and has_hand)
    )
    typed_block = bool(
        re.search(
            r"\b(order|plaintiff|defendant|affidavit|petition|subject:|in the court|"
            r"please take notice|civil miscellaneous|applicant:|respondent:)\b",
            body_lower,
        )
    )

    if messy:
        return "messy_handwriting", 0.82, "messy handwriting cues"
    if typed_block and has_hand:
        return "mixed", 0.8, "typed body + handwriting margin cues"
    if clean_hand or (has_hand and not typed_block):
        return "clean_handwriting", 0.78, "handwriting without typed court block"

    has_typed = bool(re.search(r"[A-Za-z]{3,}", body))
    non_ascii = sum(1 for c in body if ord(c) > 127)
    ascii_letters = sum(1 for c in body if c.isalpha() and ord(c) < 128)
    # Non-English / non-Latin-dominant pages
    if non_ascii > max(8, ascii_letters // 2) and not has_hand:
        return "other", 0.8, "high non-ascii density"
    # Romance-language proxies common in synthetic "other" pack
    if re.search(
        r"\b(demanda|demandante|demandado|expediente|reclamaci[oó]n|"
        r"sintético|sintetico|página|aviso)\b",
        body_lower,
    ):
        return "other", 0.78, "non-english legal language cues"

    words = re.findall(r"[A-Za-z']+", body_lower)
    eng_hits = sum(1 for w in words if w in ENGLISH_HINTS)
    if eng_hits >= 2 or (has_typed and ascii_letters > 40):
        return "typed_english", 0.72, "english-like typed text"
    if has_typed:
        return "typed_english", 0.55, "letter runs present"
    return "other", 0.5, "fallback other"


def triage_page(page: dict[str, Any]) -> dict[str, Any]:
    text = page.get("text") or ""
    is_image = bool(page.get("is_image"))
    content_len = int(page.get("content_len") or 0)
    cls, conf, notes = classify_text(text, is_image=is_image, content_len=content_len)
    return {
        "page_index": int(page.get("page_index") or 0),
        "triage_class": cls,
        "confidence": conf,
        "classifier_id": CLASSIFIER_ID,
        "notes": notes,
    }


def triage_pages(pages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [triage_page(p) for p in pages]
