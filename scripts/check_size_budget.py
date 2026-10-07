#!/usr/bin/env python3
"""Report and enforce CaseForge development/client size budgets.

B-S3-01 introduced development and client budgets. B-S4-00 adds the owner-approved
base-install cap (contracts/stage4_resource_budget.json): everything a user needs to
run the app with the DEFAULT model tier must stay under hard_cap_gib. The optional
add-on model and optional GPU accelerator libraries are reported separately and do
not count toward the base cap.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
GIB = 1024 ** 3
DEV_HARD_GIB = 9.0
DEV_WARN_GIB = 8.0
BUDGET = json.loads((ROOT / "contracts" / "stage4_resource_budget.json").read_text(encoding="utf-8"))
BASE_HARD_GIB = float(BUDGET["base_install"]["hard_cap_gib"])
BASE_TARGET_GIB = float(BUDGET["base_install"]["target_gib"])
# Ollama backend folders that are optional GPU accelerators (not shipped in the base install).
OPTIONAL_GPU_DIRS = re.compile(r"^(cuda|rocm)", re.I)


def size(path: Path) -> int:
    if not path.exists():
        return 0
    if path.is_file():
        return path.stat().st_size
    total = 0
    for entry in path.rglob("*"):
        try:
            if entry.is_file():
                total += entry.stat().st_size
        except OSError:
            continue
    return total


def manifest_model(name: str) -> str | None:
    path = ROOT / "contracts" / name
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))["model"]["name"]


def ollama_listing() -> dict[str, tuple[int, str]]:
    try:
        result = subprocess.run(["ollama", "list"], capture_output=True, text=True, timeout=10, check=False)
    except (OSError, subprocess.SubprocessError):
        return {}
    sizes: dict[str, tuple[int, str]] = {}
    for line in result.stdout.splitlines()[1:]:
        columns = line.split()
        if len(columns) < 4:
            continue
        match = re.fullmatch(r"([0-9.]+)\s*([KMGT]?B)", columns[2] + columns[3], re.I)
        if match:
            multiplier = {"B": 1, "KB": 1000, "MB": 1000**2, "GB": 1000**3, "TB": 1000**4}[match.group(2).upper()]
            sizes[columns[0]] = (int(float(match.group(1)) * multiplier), f"{columns[2]} {columns[3]}")
    return sizes


def model_bytes(model: str | None, listing: dict[str, tuple[int, str]]) -> tuple[int, str]:
    if not model:
        return 0, "no manifest"
    if model in listing:
        return listing[model][0], f"Ollama {model} ({listing[model][1]})"
    return 0, f"Ollama model {model!r} not installed/listed"


def external_speech_bytes() -> tuple[int, str]:
    configured = os.environ.get("CASEFORGE_STT_MODEL_PATH", "").strip()
    if configured:
        path = Path(configured).expanduser()
        return size(path), f"configured speech model {path}"
    hub = Path.home() / ".cache" / "huggingface" / "hub" / "models--Systran--faster-whisper-small" / "snapshots"
    snapshots = sorted((path for path in hub.glob("*") if path.is_dir()), key=lambda path: path.stat().st_mtime, reverse=True) if hub.exists() else []
    if snapshots:
        return size(snapshots[0]), "cached Systran/faster-whisper-small (client bundle requirement)"
    return 0, "Faster-Whisper Small not installed/configured"


def ollama_runtime() -> dict[str, int | str]:
    """Split the installed Ollama runtime into base (core + CPU + Vulkan) and optional GPU accelerators."""
    exe = Path(shutil.which("ollama") or "")
    if str(exe) in ("", "."):
        return {"core_bytes": 0, "optional_gpu_bytes": 0, "basis": "Ollama not installed"}
    install = exe.parent
    lib = install / "lib" / "ollama"
    optional = sum(size(d) for d in lib.iterdir() if d.is_dir() and OPTIONAL_GPU_DIRS.match(d.name)) if lib.is_dir() else 0
    total = size(install)
    return {"core_bytes": total - optional, "optional_gpu_bytes": optional,
            "basis": f"{install} (base = core + CPU + Vulkan backends; CUDA/ROCm = optional accelerators)"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    top = {p.name: size(p) for p in ROOT.iterdir()}
    workspace = sum(top.values())
    generated = sum(top.get(name, 0) for name in ("node_modules", "dist")) + size(ROOT / "src-tauri" / "target")
    listing = ollama_listing()
    default_model = os.environ.get("CASEFORGE_LLM_MODEL") or manifest_model("supported_models.json")
    addon_model = manifest_model("supported_models_addon.json")
    llm_bytes, llm_basis = model_bytes(default_model, listing)
    addon_bytes, addon_basis = model_bytes(addon_model, listing) if addon_model != default_model else (0, "same as default")
    speech_bytes, speech_basis = external_speech_bytes()
    runtime = ollama_runtime()
    runtime_core = int(runtime["core_bytes"])
    runtime_gpu = int(runtime["optional_gpu_bytes"])
    development_total = workspace + llm_bytes + addon_bytes + speech_bytes + runtime_core + runtime_gpu
    bundle = ROOT / "src-tauri" / "target" / "release" / "bundle"
    if bundle.exists():
        client = size(bundle)
        client_basis = bundle.relative_to(ROOT).as_posix()
    else:
        client_paths = [ROOT / "dist", ROOT / "sidecars", ROOT / "contracts", ROOT / "src-tauri" / "icons"]
        client = sum(size(p) for p in client_paths)
        client_basis = "pre-bundle runtime estimate (dist + sidecars + contracts + icons); final installer not built"
    base_install = client + llm_bytes + speech_bytes + runtime_core
    with_optional = base_install + addon_bytes + runtime_gpu
    failures: list[str] = []
    warnings: list[str] = []
    if development_total > DEV_HARD_GIB * GIB:
        failures.append(f"development total {development_total / GIB:.3f} GiB exceeds {DEV_HARD_GIB:.1f} GiB")
    elif development_total > DEV_WARN_GIB * GIB:
        warnings.append(f"development total {development_total / GIB:.3f} GiB is above warning level {DEV_WARN_GIB:.1f} GiB")
    if not llm_bytes:
        warnings.append(f"default model not measured ({llm_basis}); base install is incomplete")
    if base_install > BASE_HARD_GIB * GIB:
        failures.append(f"base install {base_install / GIB:.3f} GiB exceeds owner hard cap {BASE_HARD_GIB:.1f} GiB")
    elif base_install > BASE_TARGET_GIB * GIB:
        warnings.append(f"base install {base_install / GIB:.3f} GiB is above the {BASE_TARGET_GIB:.1f} GiB target (under the {BASE_HARD_GIB:.1f} GiB cap)")
    result = {
        "batch": "B-S4-00", "workspace_bytes": workspace, "workspace_gib": round(workspace / GIB, 3),
        "default_model": default_model, "external_llm_bytes": llm_bytes, "external_llm_gib": round(llm_bytes / GIB, 3),
        "external_llm_basis": llm_basis,
        "addon_model": addon_model, "addon_llm_bytes": addon_bytes, "addon_llm_gib": round(addon_bytes / GIB, 3), "addon_llm_basis": addon_basis,
        "ollama_runtime_bytes": runtime_core, "ollama_runtime_core_gib": round(runtime_core / GIB, 3),
        "ollama_optional_gpu_bytes": runtime_gpu, "ollama_optional_gpu_gib": round(runtime_gpu / GIB, 3), "ollama_basis": runtime["basis"],
        "external_speech_bytes": speech_bytes, "external_speech_gib": round(speech_bytes / GIB, 3),
        "external_speech_basis": speech_basis,
        "development_total_bytes": development_total, "development_total_gib": round(development_total / GIB, 3),
        "generated_cache_bytes": generated, "generated_cache_gib": round(generated / GIB, 3),
        "client_bytes": client, "client_gib": round(client / GIB, 3), "client_basis": client_basis,
        "base_install_bytes": base_install, "base_install_gib": round(base_install / GIB, 3),
        "with_optional_addons_gib": round(with_optional / GIB, 3),
        "client_installed_total_bytes": base_install,
        "client_installed_total_gib": round(base_install / GIB, 3),
        "budgets_gib": {"development_warning": DEV_WARN_GIB, "development_hard": DEV_HARD_GIB,
                        "base_install_target": BASE_TARGET_GIB, "base_install_hard": BASE_HARD_GIB},
        "largest_top_level": sorted(top.items(), key=lambda item: item[1], reverse=True)[:8],
        "warnings": warnings, "failures": failures, "pass": not failures,
    }
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"Repository workspace: {workspace / GIB:.3f} GiB (generated caches/builds {generated / GIB:.3f} GiB)")
        print(f"Default model: {llm_bytes / GIB:.3f} GiB ({llm_basis})")
        print(f"Offline speech model: {speech_bytes / GIB:.3f} GiB ({speech_basis})")
        print(f"Ollama runtime base: {runtime_core / GIB:.3f} GiB; optional GPU accelerators: {runtime_gpu / GIB:.3f} GiB")
        print(f"Client app payload: {client / GIB:.3f} GiB ({client_basis})")
        print(f"BASE INSTALL: {base_install / GIB:.3f} GiB (target <{BASE_TARGET_GIB:.1f} GiB; owner hard cap <{BASE_HARD_GIB:.1f} GiB)")
        print(f"Optional add-ons: model {addon_bytes / GIB:.3f} GiB ({addon_basis}); with all add-ons {with_optional / GIB:.3f} GiB")
        print(f"Development total: {development_total / GIB:.3f} GiB (hard limit {DEV_HARD_GIB:.1f} GiB)")
        for warning in warnings:
            print(f"WARN {warning}")
        for failure in failures:
            print(f"FAIL {failure}")
        if not failures:
            print("PASS size budgets (B-S3-01 development + B-S4-00 base install)")
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
