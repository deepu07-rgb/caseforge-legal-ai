/** PRD §5 New Case wizard — fields only; jurisdiction required & immutable after create. */

export const JURISDICTIONS = ["India", "United States", "Other"] as const;
export type Jurisdiction = (typeof JURISDICTIONS)[number];

export const DEFAULT_LANGUAGE = "English";

export interface CaseWizardForm {
  cause_title: string;
  court: string;
  jurisdiction: Jurisdiction | "";
  matter_type: string;
  user_side: string;
  code_era: string;
  language: string;
}

export const emptyWizard = (): CaseWizardForm => ({
  cause_title: "",
  court: "",
  jurisdiction: "",
  matter_type: "",
  user_side: "",
  code_era: "",
  language: DEFAULT_LANGUAGE,
});

/** Client-side mirror of Rust validation (jurisdiction required). */
export function validateWizard(form: CaseWizardForm): string | null {
  if (!form.cause_title.trim()) return "Cause title is required.";
  if (!form.court.trim()) return "Court is required.";
  if (!form.jurisdiction || !(JURISDICTIONS as readonly string[]).includes(form.jurisdiction)) {
    return "Jurisdiction is required (India, United States, or Other).";
  }
  if (!form.matter_type.trim()) return "Matter type is required.";
  if (!form.user_side.trim()) return "User’s side is required.";
  if (form.language.trim() !== DEFAULT_LANGUAGE) {
    return `Language must be ${DEFAULT_LANGUAGE} (Stage 1 English primary).`;
  }
  return null;
}

/** Offline stub — same intent as Rust suggest_code_era (no AI). */
export function suggestCodeEraLocal(jurisdiction: string): string {
  switch (jurisdiction) {
    case "India":
      return "India — code/era auto-suggest stub (confirm with counsel)";
    case "United States":
      return "United States — federal/state code-era stub (confirm with counsel)";
    case "Other":
      return "Other jurisdiction — code/era stub (user to complete)";
    default:
      return "";
  }
}
