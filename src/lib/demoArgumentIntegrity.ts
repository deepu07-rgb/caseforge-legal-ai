/**
 * Demo Argument Integrity (PRD §8 pedagogy).
 *
 * Loads synthetic argument trees from /demo/cases. Not GOLDEN. Not law.
 * Does not implement production Integrity Engine (B-S2-01 / Q-AN-01 still open).
 */

import easyTree from "../../demo/cases/demo_01_easy_dishonour/argument_tree.json";
import easyMeta from "../../demo/cases/demo_01_easy_dishonour/meta.json";
import mediumTree from "../../demo/cases/demo_02_medium_wrongful_term/argument_tree.json";
import mediumMeta from "../../demo/cases/demo_02_medium_wrongful_term/meta.json";
import hardTree from "../../demo/cases/demo_03_hard_partition/argument_tree.json";
import hardMeta from "../../demo/cases/demo_03_hard_partition/meta.json";

export type LinkScores = {
  Fact: number;
  Evidence: number;
  Law: number;
  Exposure: number;
};

export type StrengthGap = {
  link: string;
  current_score: number;
  gap: string;
  action: string;
};

export type DemoLimb = {
  node_id: string;
  label: string;
  theory?: string;
  links: LinkScores;
  expected_VI: number;
  strength_assessment?: {
    overall?: string;
    why?: string;
    where_argument_needs_more_strength?: StrengthGap[];
  };
};

export type DemoArgumentTree = {
  fixture_id: string;
  difficulty: string;
  status: string;
  authoritative: boolean;
  scoring_rules: {
    formula: string;
    missing_link_threshold: number;
  };
  relief_root: {
    node_id: string;
    label: string;
    jurisdiction: string;
  };
  limbs: DemoLimb[];
};

export type DemoMeta = {
  fixture_id: string;
  difficulty: string;
  jurisdiction: string;
  label: string;
  primary_teaching: string[];
  disclaimer: string;
};

export type ScoredLimb = DemoLimb & {
  minLink: keyof LinkScores;
  minScore: number;
  vi: number;
  viMatchesExpected: boolean;
  missingLinks: (keyof LinkScores)[];
};

export type DemoPack = {
  id: string;
  meta: DemoMeta;
  tree: DemoArgumentTree;
  packetPath: string;
};

const LINK_KEYS: (keyof LinkScores)[] = ["Fact", "Evidence", "Law", "Exposure"];

/** PRD §8: VI = 100 − min(Fact, Evidence, Law, Exposure) */
export function scoreLimb(limb: DemoLimb, threshold = 60): ScoredLimb {
  let minLink: keyof LinkScores = "Fact";
  let minScore = limb.links.Fact;
  for (const k of LINK_KEYS) {
    if (limb.links[k] < minScore) {
      minScore = limb.links[k];
      minLink = k;
    }
  }
  const vi = 100 - minScore;
  const missingLinks = LINK_KEYS.filter((k) => limb.links[k] < threshold);
  return {
    ...limb,
    minLink,
    minScore,
    vi,
    viMatchesExpected: vi === limb.expected_VI,
    missingLinks,
  };
}

export function scoreTree(tree: DemoArgumentTree): {
  limbs: ScoredLimb[];
  weakest: ScoredLimb | null;
  threshold: number;
} {
  const threshold = tree.scoring_rules?.missing_link_threshold ?? 60;
  const limbs = tree.limbs.map((l) => scoreLimb(l, threshold));
  const weakest =
    limbs.length === 0
      ? null
      : limbs.reduce((a, b) => (b.vi > a.vi ? b : a));
  return { limbs, weakest, threshold };
}

export const DEMO_PACKS: DemoPack[] = [
  {
    id: "demo_01_easy_dishonour",
    meta: easyMeta as DemoMeta,
    tree: easyTree as DemoArgumentTree,
    packetPath: "demo/cases/demo_01_easy_dishonour/case_packet.txt",
  },
  {
    id: "demo_02_medium_wrongful_term",
    meta: mediumMeta as DemoMeta,
    tree: mediumTree as DemoArgumentTree,
    packetPath: "demo/cases/demo_02_medium_wrongful_term/case_packet.txt",
  },
  {
    id: "demo_03_hard_partition",
    meta: hardMeta as DemoMeta,
    tree: hardTree as DemoArgumentTree,
    packetPath: "demo/cases/demo_03_hard_partition/case_packet.txt",
  },
];
