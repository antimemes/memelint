"""Write changes.md from results/corrections.json: each prompt that the model
changed, with its findings and a diff of its lines. Usage: python3 changes.py"""
import difflib, json, os

HERE = os.path.dirname(os.path.abspath(__file__))
CATS = ["qa", "reason", "arith", "track", "gsm8k", "mmlu", "follow", "long"]


def fence(lines):
    # When a line has ```, the block starts and stops with ~~~.
    mark = "~~~" if any("```" in x for x in lines) else "```"
    return [mark + "diff", *lines, mark]


def diff(r):
    out = difflib.unified_diff(r["original"].split("\n"), r["corrected"].split("\n"), lineterm="", n=0)
    return [x for x in out if not x.startswith(("---", "+++"))]


def count(n, word):
    return f"{n} {word}" if n == 1 else f"{n} {word}s"


def findings(fs):
    return [f"- `{f['rule']}` ({f['severity']}) at `{f['at'].strip() or ' '}`" for f in fs]


def main():
    recs = json.load(open(os.path.join(HERE, "results", "corrections.json")))["items"]
    md = ["# Prompt changes", "",
          "This file has each prompt that the model changed for the run in [README.md](README.md). For each prompt, "
          "it gives the memelint findings before the change and a diff of the lines that changed. "
          "[`changes.py`](changes.py) writes this file from "
          "[`results/corrections.json`](results/corrections.json).", "",
          "| Category | Prompts | Prompts with findings | Changed | Findings before | Findings after |",
          "|---|---:|---:|---:|---:|---:|"]
    for c in CATS + ["total"]:
        rs = [r for r in recs if c in ("total", r["category"])]
        md.append(f"| `{c}` | {len(rs)} | {sum(bool(r['before']) for r in rs)} | "
                  f"{sum(r['corrected'] != r['original'] for r in rs)} | {sum(len(r['before']) for r in rs)} | "
                  f"{sum(len(r['after']) for r in rs)} |")
    for c in CATS:
        rs = [r for r in recs if r["category"] == c and r["corrected"] != r["original"]]
        if not rs:
            continue
        md += ["", f"## `{c}`", ""]
        for r in rs:
            md += [f"### `{r['id']}`", "",
                   f"Status `{r['status']}`, {count(len(r['before']), 'finding')} before, {len(r['after'])} after, "
                   f"{count(len(r['attempts']), 'request')} to the model.", "", "Findings before:", "", *findings(r["before"])]
            if r["after"]:
                md += ["", "Findings after:", "", *findings(r["after"])]
            md += ["", *fence(diff(r)), ""]
    kept = [r for r in recs if r["status"] == "kept"]
    md += ["", "## Prompts that did not change", "",
           "These prompts had findings, but the harness kept the initial text. A reply of the model is "
           "`rejected` when the guard of the harness found a change to the task. If not, the number is the "
           "findings of the reply. The harness keeps a reply only when it has a smaller number of findings than "
           "the initial prompt. Refer to [README.md](README.md).", ""]
    for r in kept:
        md += [f"### `{r['id']}`", "", *findings(r["before"]), "", "Requests to the model:", ""]
        md += [f"{n}. rejected: `{x['rejected']}`" if x["rejected"] else f"{n}. {count(len(x['findings']), 'finding')}"
               for n, x in enumerate(r["attempts"], 1)]
        md.append("")
    with open(os.path.join(HERE, "changes.md"), "w") as f:
        f.write("\n".join(md).rstrip() + "\n")


main()
