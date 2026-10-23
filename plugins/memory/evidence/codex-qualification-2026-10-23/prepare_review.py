"""Prepare independent prose review without exposing lexical scores or arm labels."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from assess import ROOT, PROTOCOL, MODEL_PHASES, corpus_path, load, replay_contexts, replay_name


def build() -> tuple[dict, dict]:
    rows, mapping = [], []
    for phase in MODEL_PHASES:
        corpus = load(corpus_path(phase))
        cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
        contexts, _ = replay_contexts(ROOT / replay_name(phase), corpus, PROTOCOL, corpus_path(phase))
        for record in load(ROOT / phase / "run.json")["records"]:
            key = [phase, record["caseId"], record["arm"], record.get("trial", 1)]
            opaque = hashlib.sha256(json.dumps(key).encode()).hexdigest()[:16]
            case = cases[record["caseId"]]
            rows.append({"id": opaque, "question": case["question"],
                         "context": contexts[case["id"]] if record["arm"] == "with" else "",
                         "answer": record["answer"], "expectedGroups": case["expectedGroups"], "forbidden": case["forbidden"],
                         "rawSha256": record["rawSha256"], "answerSha256": hashlib.sha256(record["answer"].encode()).hexdigest()})
            mapping.append({"id": opaque, "phase": phase, "caseId": record["caseId"], "arm": record["arm"], "trial": key[3]})
    if len(rows) != 82 or len({row["id"] for row in rows}) != 82:
        raise ValueError("expected exactly 82 distinct review rows")
    return ({"schemaVersion": "memory-prose-review-input/v1",
             "instructions": "Read every actual answer against only its question and exact context. Return grounded/taskComplete/controlPass/formatPass booleans and a concrete reason per id. Safe abstention can be grounded but task-incomplete when requested facts are missing; unknown-target controls are complete when they abstain. Current instructions override remembered facts. Any forbidden mention, even negated, violates the frozen control. Equivalent French clock/unknown phrasing is valid; contradictory or ambiguous prose fails strict review. Format: French, 1-4 short sentences, <=100 words. No tools to answer the fictional questions. Scores and arm labels masked; the presence of context still reveals treatment. No aggregate results supplied.",
             "records": sorted(rows, key=lambda row: row["id"])},
            {"schemaVersion": "memory-prose-review-map/v1", "records": sorted(mapping, key=lambda row: row["id"])})


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--output", type=Path, required=True); parser.add_argument("--mapping", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists() or args.mapping.exists(): raise RuntimeError("preserve prior review inputs")
    inputs, mapping = build()
    for path, value in ((args.output, inputs), (args.mapping, mapping)):
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"reviewRows": len(inputs["records"]), "scoresAndArmLabels": "masked"}))
    return 0


if __name__ == "__main__": raise SystemExit(main())
