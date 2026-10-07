/**
 * Bundled synthetic demo packs for one-click vault seed.
 * Not GOLDEN. Not law.
 */

import easyPacket from "../../demo/cases/demo_01_easy_dishonour/case_packet.txt?raw";
import mediumPacket from "../../demo/cases/demo_02_medium_wrongful_term/case_packet.txt?raw";
import hardPacket from "../../demo/cases/demo_03_hard_partition/case_packet.txt?raw";
import type { CaseWizardInput } from "./ipc";

export type DemoSeedId =
  | "demo_01_easy_dishonour"
  | "demo_02_medium_wrongful_term"
  | "demo_03_hard_partition";

export type DemoSeed = {
  id: DemoSeedId;
  difficulty: "easy" | "medium" | "hard";
  label: string;
  wizard: CaseWizardInput;
  packetFilename: string;
  packetText: string;
};

export const DEMO_SEEDS: DemoSeed[] = [
  {
    id: "demo_01_easy_dishonour",
    difficulty: "easy",
    label: "Easy — cheque dishonour (India)",
    wizard: {
      cause_title: "M/s Blue River Traders v. Rohan Mehta",
      court: "Commercial Court / Magistrate Court (teaching stub)",
      jurisdiction: "India",
      matter_type: "Negotiable Instruments — cheque dishonour",
      user_side: "Plaintiff / Complainant",
      code_era: "India — NI Act teaching stub",
      language: "English",
    },
    packetFilename: "case_packet_demo_01_easy.txt",
    packetText: easyPacket,
  },
  {
    id: "demo_02_medium_wrongful_term",
    difficulty: "medium",
    label: "Medium — wrongful termination (US)",
    wizard: {
      cause_title: "Jordan Cole v. Northline Logistics Inc.",
      court: "United States District Court (teaching stub)",
      jurisdiction: "United States",
      matter_type: "Employment — wrongful termination / unpaid bonus",
      user_side: "Plaintiff",
      code_era: "United States — federal/state teaching stub",
      language: "English",
    },
    packetFilename: "case_packet_demo_02_medium.txt",
    packetText: mediumPacket,
  },
  {
    id: "demo_03_hard_partition",
    difficulty: "hard",
    label: "Hard — partition / oral settlement (India)",
    wizard: {
      cause_title: "Kavita Sharma & Anr. v. Vikram Sharma",
      court: "Civil Court / ADJ (teaching stub)",
      jurisdiction: "India",
      matter_type: "Partition / declaration / HUF property",
      user_side: "Plaintiff",
      code_era: "India — property / HUF teaching stub",
      language: "English",
    },
    packetFilename: "case_packet_demo_03_hard.txt",
    packetText: hardPacket,
  },
];

/** UTF-8 string → standard base64 for doc_ingest. */
export function textToBase64(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]!);
  }
  return btoa(binary);
}
