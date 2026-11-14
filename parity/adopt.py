"""First status for newly mapped cases (design § Ratchet, bootstrap).

usage: python3 -I parity/adopt.py parity/report/results/<suite>.json [...]
       python3 -I parity/adopt.py --reset <suite>   (provisional statuses only:
       back to unmapped before a suite's first commit, e.g. after a mapping fix)

For each result whose catalogue status is `unmapped`, writes the suite into
`tests:` and the outcome as the first status (pass → green, fail → red).
Entries already red/green/waived are never touched: after adoption the
ratchet owns them. Run a suite with PARITY_ADOPT=1 to produce the results.
"""
import json
import re
import sys
from pathlib import Path

CAT = Path(__file__).resolve().parent / "catalogue"


def reset(suite):
    n = 0
    for f in sorted(CAT.glob("*.yaml")):
        text = f.read_text()
        parts = re.split(r"(?m)^(?=- id: )", text)
        for i, block in enumerate(parts):
            if re.search(rf"(?m)^  tests: \[{re.escape(suite)}\][ \t]*$", block) and re.search(r"(?m)^  status: (red|green)[ \t]*$", block):
                block = re.sub(r"(?m)^  status: (red|green)[ \t]*$", "  status: unmapped", block)
                parts[i] = re.sub(r"(?m)^  tests: \[.*\][ \t]*$", "  tests: []", block)
                n += 1
        new = "".join(parts)
        if new != text:
            f.write_text(new)
    print(f"reset {n} entries of {suite}")


def main(paths):
    outcome = {}
    for p in paths:
        suite = Path(p).stem.replace("__", "::")
        for r in json.loads(Path(p).read_text()):
            if r["status"] == "unmapped":
                outcome[r["id"]] = (suite, "green" if r["pass"] else "red")
    changed = 0
    for f in sorted(CAT.glob("*.yaml")):
        text = f.read_text()
        # entries start at "- id: X"; edit only status/tests lines inside that block
        parts = re.split(r"(?m)^(?=- id: )", text)
        out = []
        for block in parts:
            m = re.match(r"- id: (\S+)", block)
            if m and m.group(1) in outcome and re.search(r"(?m)^  status: unmapped[ \t]*$", block):
                suite, status = outcome[m.group(1)]
                block = re.sub(r"(?m)^  status: unmapped[ \t]*$", f"  status: {status}", block)
                block, n = re.subn(r"(?m)^  tests: \[\][ \t]*$", f"  tests: [{suite}]", block)
                if n != 1:
                    sys.exit(f"{m.group(1)} in {f.name}: expected one 'tests: []' line")
                changed += 1
            out.append(block)
        new = "".join(out)
        if new != text:
            f.write_text(new)
    print(f"adopted {changed} of {len(outcome)} results")


if __name__ == "__main__":
    if sys.argv[1:2] == ["--reset"]:
        reset(sys.argv[2])
    else:
        main(sys.argv[1:])
