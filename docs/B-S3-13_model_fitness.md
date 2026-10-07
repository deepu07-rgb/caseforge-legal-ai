# B-S3-13 — Local model fitness and controlled-beta freeze

## Frozen development configuration

The only controlled-beta configuration is the exact entry in `contracts/supported_models.json`. The current audited runtime is Ollama 0.32.14 with `qwen2.5:7b`, digest `sha256:845dbda0…b697e`, 7.6B parameters, Q4_K_M quantization, and a declared 32,768-token model context. CaseForge intentionally runs an 8,192-token context to stay within the development hardware envelope. Ollama 0.32.14 replaced the previously audited 0.32.9 runtime on 2026-08-18; readiness remained fail-closed until the full fitness suite was rerun.

This is accurately classified as **stock-instruct + quantized + CaseForge-prompted**. It is not described as legally fine-tuned: no adapter, legal fine-tune checkpoint, training recipe, or checkpoint digest exists in the repository.

The CaseForge system prompt is a versioned contract. Every generated result records the model digest, prompt version, system-prompt hash, full assembled-prompt hash, generation parameters, working-set version, authority IDs, context sizes, and whether a bounded case excerpt was used.

## Fail-closed behavior

- A running model is not the same as controlled-beta readiness. Backend, runtime version, tag, digest, family, parameter count, quantization, model context, licence metadata, and prompt hash must all match the allowlist.
- Unsupported OpenAI-compatible and llama.cpp configurations remain available only for engineering when `CASEFORGE_ALLOW_UNVALIDATED_MODEL=1`; otherwise generation is denied and they cannot claim readiness.
- Raw case context over 24,000 characters and authority context over 16,000 characters are rejected. Input is never silently sliced.
- A bounded vault excerpt is visibly marked. Generation requires a counsel-controlled matter working-set version alongside it.
- Only selected authority IDs and exact selected authority text are supplied to the model. The prompt requires an explicit `NOT VERIFIED` response when controlling law is absent.

## Evaluation and halt rule

Run `npm run test:s3:13`. The live suite covers fact grounding, citation grounding, unknown law, contradictions, opposition behavior, prompt injection, nonexistent authorities, quotation fidelity, source inspection, provenance, and long-context refusal. The gate requires at least 85%, every critical safety check, an exact allowlisted configuration, warm-turn latency no higher than 60 seconds, cold start no higher than 120 seconds, and an Ollama resident allocation no higher than 6.5 GB. Cold start is reported separately because loading 4.36 GiB of weights is materially different from an already-resident turn.

Any unknown licence, digest, context capacity, prompt drift, unsupported configuration, critical safety failure, or performance failure is a controlled-beta **HALT**, not a warning that can be ignored.

## Hardware and distribution size

The audited development machine has an Intel Core i5-11400H, 16 GB RAM, and an NVIDIA RTX 3050 Laptop GPU with 4 GB VRAM. The installed Ollama model is about 4.36 GiB by itself. Therefore a client bundle below 4 GiB cannot include this model. The current development limit remains below 8–9 GiB combined; a later sub-4-GiB client target requires a separately evaluated smaller model or model-on-demand distribution and must not inherit this readiness claim.
