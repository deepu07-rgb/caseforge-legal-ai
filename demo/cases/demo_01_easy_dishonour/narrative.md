# Demo 01 — Easy: cheque dishonour (India)

**Status:** `DEMO_SYNTHETIC` · **Not law** · **Not GOLDEN**

## Story (one paragraph)

Blue River Traders sold goods; Rohan Mehta issued a cheque for the invoice; the bank returned it for insufficient funds; a demand notice went out and was (in this demo) received; nothing was paid; a complaint was filed on time.

## Why this is “easy”

| Link | Score | Why high |
|------|------:|----------|
| Fact | 92 | Single debt story, uncontested in the packet |
| Evidence | 90 | Invoice + cheque + memo + notice packet |
| Law | 88 | Elements of the teaching NI Act s.138 path line up |
| Exposure | 85 | No serious counterclaim planted |

**VI ≈ 15** → strong limb. Missing Links should be empty at threshold 60.

## Where to strengthen (still useful)

Even strong cases fail in court on small process points. Here, the only “more strength” items are housekeeping: bind service proof and element checklist.

## How to test in CaseForge

1. Create India case in wizard.
2. Ingest `case_packet.txt`.
3. Run triage.
4. Score tree: `npm run demo:score`.
5. Confirm weakest VI is on L1 and low.
