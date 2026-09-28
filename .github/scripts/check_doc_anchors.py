"""Check the stable section anchors in the docs.

Every "##" and "###" heading in the documents below must carry an anchor such as
`<a id="spec-7-control-set"></a>`. Anchor ids must be unique, and every link in the
repository's Markdown files that points at one of these ids must resolve.

Ids are never derived from the heading text here on purpose: issues link to them, so an
id must stay the same when its heading is reworded. See CONTRIBUTING.md.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
DOCS = {
    "docs/spec/technical_spec.md": "spec",
    "docs/architecture.md": "arch",
    "docs/spikes/linkme-registry.md": "spike-linkme",
}
MARKER = re.compile(r'<a id="([a-z0-9-]+)"></a>')
HEADING = re.compile(r"^#{2,3} ")
LINK = re.compile(r"\]\(([^)\s]*)#((?:spec|arch|spike-linkme)-[a-z0-9-]+)\)")


def headings(path):
    in_fence = False
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
        elif not in_fence and HEADING.match(line):
            yield number, line


def main():
    errors = []
    anchors = {}  # id -> (doc, line)
    for doc, prefix in DOCS.items():
        for number, line in headings(ROOT / doc):
            found = MARKER.findall(line)
            if not found:
                errors.append(f"{doc}:{number}: heading has no anchor: {line.strip()}")
                continue
            for anchor in found:
                if not anchor.startswith(prefix + "-") and anchor != prefix:
                    errors.append(f"{doc}:{number}: anchor '{anchor}' must start with '{prefix}-'")
                if anchor in anchors:
                    other = anchors[anchor]
                    errors.append(f"{doc}:{number}: anchor '{anchor}' already used at {other[0]}:{other[1]}")
                anchors[anchor] = (doc, number)

    for md in sorted(ROOT.rglob("*.md")):
        rel = md.relative_to(ROOT).as_posix()
        if rel.startswith(("target/", "spikes/")) or "/target/" in rel:
            continue
        for number, line in enumerate(md.read_text(encoding="utf-8").splitlines(), 1):
            for target, anchor in LINK.findall(line):
                doc = (md.parent / target).resolve().relative_to(ROOT).as_posix() if target else rel
                if target.startswith("http"):
                    doc = target.split("/blob/main/", 1)[-1]
                if anchor not in anchors:
                    errors.append(f"{rel}:{number}: link to unknown anchor '#{anchor}'")
                elif anchors[anchor][0] != doc:
                    errors.append(f"{rel}:{number}: '#{anchor}' lives in {anchors[anchor][0]}, not {doc}")

    for error in errors:
        print(error)
    print(f"{len(anchors)} anchors checked, {len(errors)} problems")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
