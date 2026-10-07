# B-S3-10 — First authoritative legal repository

## Product-owner selection

Selected on 2026-08-16:

- Jurisdiction: India (central law only)
- Court level: High Court
- Matter type: property partition dispute
- Procedural posture: civil appeal

The controlled pack is intentionally narrower than general Indian property law. The proposed
baseline covers the Partition Act, 1893, the civil-appeal provisions of the Code of Civil Procedure,
1908, and the relevant appeal provisions of the Limitation Act, 1963. State amendments, individual
High Court rules, agricultural/revenue land, and questions whose answer depends on an unselected
personal-law regime are outside the pack and must produce a repository-gap result.

## Authoritative-source review

Reviewed on 2026-08-16:

| Candidate | Issuing authority | Official record | Authority/currentness result | Redistribution result |
|---|---|---|---|---|
| Partition Act, 1893 | Ministry of Law and Justice, Legislative Department | https://www.indiacode.nic.in/handle/123456789/5657 | Authoritative India Code record and official PDF located | Section 52(1)(q)(ii) basis; accompanied by original CaseForge matter |
| Code of Civil Procedure, 1908 | Ministry of Law and Justice, Legislative Department | https://www.indiacode.nic.in/handle/123456789/2191 | Authoritative India Code record located; central text carries state-amendment caveats | Section 52(1)(q)(ii) basis; accompanied by original CaseForge matter |
| Limitation Act, 1963 | Ministry of Law and Justice, Legislative Department | https://www.indiacode.nic.in/indiacode/handle/123456789/1565 | Authoritative India Code record and official PDF located | Section 52(1)(q)(ii) basis; accompanied by original CaseForge matter |

India Code's disclaimer states that its PDFs are exact reproductions of officially adopted text,
subject to the English version prevailing in case of discrepancy. The repository does not rely on an
India Code site licence or on a general NDSAP inference; it relies on the specific statutory exception
in Copyright Act section 52(1)(q)(ii) and satisfies its original-matter condition.

## Rights basis and gate resolution

The product owner confirmed that Acts and Rules may be used in the knowledge repository. The
specific statutory basis was then verified against the current official Copyright Act, 1957:
section 52(1)(q)(ii) provides that reproduction or publication of an Act of a Legislature is not
infringement when the Act is reproduced or published together with commentary or other original
matter. The pack therefore includes CaseForge-authored scope commentary, applicability limitations,
provenance, update rules, and retrieval questions. It does not copy India Code logos or site layout
and does not claim government endorsement.

## Implemented controlled pack

- Pack ID: `caseforge.india.partition.highcourt.civil-appeal`
- Version: `1.0.0-beta.1`
- Sources: Partition Act, 1893; selected CPC appeal provisions and Order XLI pages; selected
  Limitation Act provisions and Schedule Articles 115-117
- Lineage: 20 exact-text page records and 20 retrieval chunks
- Governance: official URLs, acquisition date, PDF SHA-256 values, original commentary, editorial
  checklist, monthly freshness rule, supersession fields, and deliberately absent-authority goldens
- Trust: Ed25519 development signature; release-key rotation remains mandatory before client ship

AC-S3-10.1 through AC-S3-10.5 are implemented for controlled development testing. Named legal-editor
approval and the release signing-key ceremony remain client-distribution gates; they do not permit
the model to fill a repository gap.
