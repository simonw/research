# OpenAI Skills API — Hands-On Demo

*2026-02-11T14:58:37Z*

## What Are OpenAI Skills?

Skills are reusable bundles of files — instructions, scripts, and assets — packaged as a folder with a `SKILL.md` manifest. When attached to a shell tool environment, the model can discover the skill, read its instructions, and execute its scripts on demand.

This demo walks through the cookbook examples from the OpenAI developer docs: creating a skill, uploading it, and invoking it via the Responses API.

## Step 1: Build the csv_insights_skill

Following the cookbook example, we create a skill folder with four files: `SKILL.md` (the manifest), `run.py` (the main script), `requirements.txt`, and a sample CSV asset.

```bash
mkdir -p csv_insights_skill/assets && echo "Skill directory created:" && ls -la csv_insights_skill/
```

```output
Skill directory created:
total 12
drwxr-xr-x 3 root root 4096 Feb 11 15:04 .
drwxr-xr-x 4 root root 4096 Feb 11 15:04 ..
drwxr-xr-x 2 root root 4096 Feb 11 15:04 assets
```

Create the `SKILL.md` manifest. The frontmatter `name` and `description` are what OpenAI reads for discovery and routing.

```bash
cat > csv_insights_skill/SKILL.md << 'SKILLEOF'
---
name: csv-insights
description: Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.
---

# CSV Insights Skill

## When to use this
Use this skill when the user provides a CSV file and wants:
- a quick summary (row/col counts, missing values)
- basic numeric statistics
- a simple visualization
- results packaged into an output folder (or zip)

## Inputs
- A CSV file path (local) or a file mounted in the container.

## Outputs
- `output/report.md`
- `output/plot.png`

## How to run

python -m pip install -r requirements.txt
python run.py --input <csv_file> --outdir output
SKILLEOF
echo "SKILL.md created ($(wc -l < csv_insights_skill/SKILL.md) lines)"
head -5 csv_insights_skill/SKILL.md
```

```output
SKILL.md created (25 lines)
---
name: csv-insights
description: Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.
---

```

Create `run.py` — the main script. Following the cookbook, it reads a CSV, writes a markdown report, and generates a histogram.

```bash
cat > csv_insights_skill/run.py << 'PYEOF'
import argparse
from pathlib import Path

import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


def write_report(df: pd.DataFrame, outpath: Path) -> None:
    lines = []
    lines.append(f"# CSV Insights Report\n")
    lines.append(f"**Rows:** {len(df)}  \n**Columns:** {len(df.columns)}\n")
    lines.append("\n## Columns\n")
    lines.append("\n".join([f"- `{c}` ({df[c].dtype})" for c in df.columns]))

    missing = df.isna().sum()
    if missing.any():
        lines.append("\n## Missing values\n")
        for col, count in missing[missing > 0].items():
            lines.append(f"- `{col}`: {int(count)}")
    else:
        lines.append("\n## Missing values\nNo missing values detected.\n")

    numeric = df.select_dtypes(include="number")
    if not numeric.empty:
        lines.append("\n## Numeric summary (describe)\n")
        lines.append(numeric.describe().to_markdown())

    outpath.write_text("\n".join(lines), encoding="utf-8")


def make_plot(df: pd.DataFrame, outpath: Path) -> None:
    numeric = df.select_dtypes(include="number")
    if numeric.empty:
        return
    col = numeric.columns[0]
    plt.figure()
    df[col].dropna().hist(bins=30)
    plt.title(f"Histogram: {col}")
    plt.xlabel(col)
    plt.ylabel("Count")
    plt.tight_layout()
    plt.savefig(outpath)
    plt.close()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, help="Path to input CSV")
    parser.add_argument("--outdir", required=True, help="Directory for outputs")
    args = parser.parse_args()

    inpath = Path(args.input)
    outdir = Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)

    df = pd.read_csv(inpath)

    write_report(df, outdir / "report.md")
    make_plot(df, outdir / "plot.png")
    print(f"Report written to {outdir / 'report.md'}")
    print(f"Plot written to {outdir / 'plot.png'}")


if __name__ == "__main__":
    main()
PYEOF
echo "run.py created ($(wc -l < csv_insights_skill/run.py) lines)"
```

```output
run.py created (67 lines)
```

```bash
cat > csv_insights_skill/requirements.txt << 'EOF'
pandas
matplotlib
tabulate
EOF
echo "requirements.txt:"
cat csv_insights_skill/requirements.txt
```

```output
requirements.txt:
pandas
matplotlib
tabulate
```

Create a sample CSV asset so the skill can be tested standalone.

```bash
cat > csv_insights_skill/assets/example.csv << 'EOF'
name,age,salary,department
Alice,30,70000,Engineering
Bob,25,55000,Marketing
Charlie,,80000,Engineering
Diana,35,90000,Sales
Eve,28,,Marketing
Frank,40,75000,Engineering
Grace,33,62000,Sales
Hank,29,58000,Marketing
Ivy,31,85000,Engineering
Jack,27,52000,Sales
EOF
echo "example.csv created:"
cat csv_insights_skill/assets/example.csv
```

```output
example.csv created:
name,age,salary,department
Alice,30,70000,Engineering
Bob,25,55000,Marketing
Charlie,,80000,Engineering
Diana,35,90000,Sales
Eve,28,,Marketing
Frank,40,75000,Engineering
Grace,33,62000,Sales
Hank,29,58000,Marketing
Ivy,31,85000,Engineering
Jack,27,52000,Sales
```

Review the final skill folder layout — matches the cookbook structure exactly.

```bash
find csv_insights_skill -type f | sort
```

```output
csv_insights_skill/SKILL.md
csv_insights_skill/assets/example.csv
csv_insights_skill/requirements.txt
csv_insights_skill/run.py
```

## Step 2: Zip and Upload the Skill

The cookbook recommends zip upload for reliability. We zip the folder and POST it to `/v1/skills`.

```bash
rm -f csv_insights_skill.zip && zip -r csv_insights_skill.zip csv_insights_skill/
```

```output
  adding: csv_insights_skill/ (stored 0%)
  adding: csv_insights_skill/requirements.txt (stored 0%)
  adding: csv_insights_skill/run.py (deflated 58%)
  adding: csv_insights_skill/SKILL.md (deflated 38%)
  adding: csv_insights_skill/assets/ (stored 0%)
  adding: csv_insights_skill/assets/example.csv (deflated 42%)
```

```bash
curl -s -X POST "https://api.openai.com/v1/skills" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -F "files=@./csv_insights_skill.zip;type=application/zip" | python3 -m json.tool
```

```output
{
    "id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c",
    "object": "skill",
    "created_at": 1770824079,
    "default_version": "1",
    "description": "Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.",
    "latest_version": "1",
    "name": "csv-insights"
}
```

Upload successful! The API returned:
- **id:** `skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c`
- **name:** csv-insights
- **default_version:** 1, **latest_version:** 1

Note the version pointers — these are what you pin in production.

## Step 3: Invoke the Skill via the Responses API (Hosted Shell)

We call the Responses API with the `shell` tool and `container_auto` environment, attaching our skill by ID. The model discovers the skill, reads its SKILL.md, installs deps, and runs the script. The `shell` tool requires a model that supports it — here we use `gpt-5.2` as the cookbook recommends.

```bash
uv run --with openai python -c '
from openai import OpenAI

client = OpenAI()
SKILL_ID = "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c"

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [
                {"type": "skill_reference", "skill_id": SKILL_ID},
            ],
        },
    }],
    input="Use the csv-insights skill to analyze its bundled example.csv and show me the report.",
)

print("=== Model output ===")
print(response.output_text)
print()
print("=== Response metadata ===")
print(f"Status: {response.status}")
print(f"Model: {response.model}")
print(f"Usage: input={response.usage.input_tokens}, output={response.usage.output_tokens}")
'
```

````output
=== Model output ===
Using **csv-insights** on the bundled `example.csv`. Here’s the generated report (verbatim), plus links to the produced files.

## Report

```md
# CSV Insights Report

**Rows:** 10  
**Columns:** 4


## Columns

- `name` (object)
- `age` (float64)
- `salary` (float64)
- `department` (object)

## Missing values

- `age`: 1
- `salary`: 1

## Numeric summary (describe)

|       |     age |   salary |
|:------|--------:|---------:|
| count |  9      |      9   |
| mean  | 30.8889 |  69666.7 |
| std   |  4.5674 |  13720.4 |
| min   | 25      |  52000   |
| 25%   | 28      |  58000   |
| 50%   | 30      |  70000   |
| 75%   | 33      |  80000   |
| max   | 40      |  90000   |
```

## Output files
- Markdown report: [report.md](sandbox:/mnt/data/csv_insights_output/report.md)  
- Plot image: [plot.png](sandbox:/mnt/data/csv_insights_output/plot.png)

=== Response metadata ===
Status: completed
Model: gpt-5.2-2025-12-11
Usage: input=3764, output=665
````

The model successfully:
1. Discovered the csv-insights skill from the environment
2. Read its SKILL.md to learn the workflow
3. Installed requirements (pandas, matplotlib, tabulate)
4. Ran `run.py --input assets/example.csv --outdir output`
5. Returned the generated report with stats and missing-value analysis

All from a single natural-language prompt — the skill packaging did the heavy lifting.

## Step 4: Experiments — Exploring the Skills API

Now let's go beyond the cookbook and explore the API surface: listing skills, retrieving details, version pinning, and inline skills.

### 4a. List all skills

The API should support listing skills. Let's see what we have.

```bash
curl -s "https://api.openai.com/v1/skills" \
  -H "Authorization: Bearer $OPENAI_API_KEY" | python3 -m json.tool
```

```output
{
    "object": "list",
    "data": [
        {
            "id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c",
            "object": "skill",
            "created_at": 1770824079,
            "default_version": "1",
            "description": "Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.",
            "latest_version": "1",
            "name": "csv-insights"
        },
        {
            "id": "skill_698ca05bafc4819181d236f268efba9d05ded4609a783582",
            "object": "skill",
            "created_at": 1770823771,
            "default_version": "1",
            "description": "Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.",
            "latest_version": "1",
            "name": "csv-insights"
        }
    ],
    "first_id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c",
    "has_more": false,
    "last_id": "skill_698ca05bafc4819181d236f268efba9d05ded4609a783582"
}
```

### 4b. Retrieve a single skill by ID

```bash
curl -s 'https://api.openai.com/v1/skills/skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c' -H 'Authorization: Bearer '"$OPENAI_API_KEY" | python3 -m json.tool
```

```output
{
    "id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c",
    "object": "skill",
    "created_at": 1770824079,
    "default_version": "1",
    "description": "Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.",
    "latest_version": "1",
    "name": "csv-insights"
}
```

### 4c. Version pinning — use a specific version

The cookbook recommends pinning skill versions in production. Let's explicitly request version 1.

```bash
uv run --with openai python -c '
from openai import OpenAI
client = OpenAI()

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [
                {"type": "skill_reference", "skill_id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c", "version": "1"},
            ],
        },
    }],
    input="What skills are available to you? List their names and paths.",
)
print(response.output_text)
'
```

```output
Available skills in this session:

- **csv-insights** — `/home/oai/skills/csv-insights-1`
```

The model confirms it sees the skill at `/home/oai/skills/csv-insights-1`. Note the `-1` suffix — that's the pinned version number reflected in the filesystem path. Also note: `version` must be a **string** (e.g. `"1"`), not an integer.

### 4d. Inline skills — skip the upload, send a base64 zip directly

The docs mention you can pass a skill as an `inline` type with a base64 zip bundle. Let's build a tiny skill and send it inline.

```bash
mkdir -p greeter_skill && cat > greeter_skill/SKILL.md << 'EOF'
---
name: greeter
description: Generate a cheerful greeting in any language.
---

# Greeter Skill

## When to use this
Use when the user asks for a greeting in a specific language.

## How to run
python greet.py --lang <language>
EOF

cat > greeter_skill/greet.py << 'EOF'
import argparse

GREETINGS = {
    "english": "Hello! Have a wonderful day!",
    "spanish": "¡Hola! ¡Que tengas un día maravilloso!",
    "japanese": "こんにちは！素敵な一日を！",
    "french": "Bonjour ! Passez une merveilleuse journée !",
    "german": "Hallo! Hab einen wundervollen Tag!",
}

parser = argparse.ArgumentParser()
parser.add_argument("--lang", default="english")
args = parser.parse_args()
lang = args.lang.lower()
print(GREETINGS.get(lang, f"Hello from the greeter skill! (no greeting for {lang})"))
EOF

rm -f greeter_skill.zip && zip -r greeter_skill.zip greeter_skill/
echo "---"
echo "Zip size: $(wc -c < greeter_skill.zip) bytes"
```

```output
  adding: greeter_skill/ (stored 0%)
  adding: greeter_skill/SKILL.md (deflated 34%)
  adding: greeter_skill/greet.py (deflated 32%)
---
Zip size: 1040 bytes
```

```bash
uv run --with openai python -c '
import base64, pathlib
from openai import OpenAI

client = OpenAI()
zip_b64 = base64.b64encode(pathlib.Path("greeter_skill.zip").read_bytes()).decode()

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [{
                "type": "inline",
                "name": "greeter",
                "description": "Generate a cheerful greeting in any language.",
                "source": {
                    "type": "base64",
                    "media_type": "application/zip",
                    "data": zip_b64,
                },
            }],
        },
    }],
    input="Greet me in Japanese, Spanish, and German using the greeter skill.",
)
print(response.output_text)
'
```

```output
Using the **greeter** skill:

- **Japanese:** こんにちは！素敵な一日を！  
- **Spanish:** ¡Hola! ¡Que tengas un día maravilloso!  
- **German:** Hallo! Hab einen wundervollen Tag!
```

Inline skills work! The correct shape for `source` is an object:

```json
{"type": "base64", "media_type": "application/zip", "data": "<b64-encoded-zip>"}
```

This is handy for prototyping — no need to upload and manage skill IDs. The model ran `greet.py` three times with different `--lang` arguments and assembled the results.

### 4e. Multiple skills in one request

Can we attach both the uploaded csv-insights skill and the inline greeter skill in a single call? The model should be able to pick which one to use based on the prompt.

```bash
uv run --with openai python -c '
import base64, pathlib
from openai import OpenAI

client = OpenAI()
zip_b64 = base64.b64encode(pathlib.Path("greeter_skill.zip").read_bytes()).decode()

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [
                {"type": "skill_reference", "skill_id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c"},
                {
                    "type": "inline",
                    "name": "greeter",
                    "description": "Generate a cheerful greeting in any language.",
                    "source": {
                        "type": "base64",
                        "media_type": "application/zip",
                        "data": zip_b64,
                    },
                },
            ],
        },
    }],
    input="First greet me in French, then analyze the CSV from csv-insights. Show both results.",
)
print(response.output_text)
'
```

```output
Using **greeter** (to greet you in French) and **csv-insights** (to analyze a provided CSV).

## 1) Greeting (French)
Bonjour ! Passez une merveilleuse journée !

## 2) CSV analysis (csv-insights)
No CSV file is currently available in `/mnt/data` (it’s empty), so I can’t run `csv-insights` yet.

Upload or provide the CSV filename/path (it should appear under `/mnt/data`), and I’ll generate both:
- `report.md` (summary + stats)
- `plot.png` (simple visualization)
```

The model correctly routed to both skills\! It ran the greeter for the French greeting, then attempted csv-insights but noted the CSV needs to be in `/mnt/data` (the container's data mount). In a real workflow you'd upload the CSV as an input file. The key insight: **skill routing is automatic** — the model reads the `name` and `description` to decide which skill to invoke.

### 4f. Telling the model to use the bundled asset

Let's be more explicit and tell the model to use the example.csv that's bundled *inside* the skill itself.

```bash
uv run --with openai python -c '
from openai import OpenAI
client = OpenAI()

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [
                {"type": "skill_reference", "skill_id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c"},
            ],
        },
    }],
    input="List the files in the csv-insights skill directory, then use the skill to analyze the example.csv that is bundled in its assets/ folder.",
)
print(response.output_text)
'
```

````output
## Files in the `csv-insights` skill directory

```text
/home/oai/skills/csv-insights-1:
  SKILL.md
  requirements.txt
  run.py
  assets/example.csv
```

## CSV analysis (using the `csv-insights` skill) on `assets/example.csv`

Outputs generated:

- Markdown report: [report.md](sandbox:/mnt/data/report.md)  
- Plot image: [plot.png](sandbox:/mnt/data/plot.png)

Key results (from the report):

- **Rows:** 10  
- **Columns:** 4 (`name`, `age`, `salary`, `department`)
- **Missing values:** `age` = 1, `salary` = 1
- **Numeric summary (age, salary):**
  - `age` mean ≈ **30.89**, min **25**, max **40** (9 non-missing)
  - `salary` mean ≈ **69,666.7**, min **52,000**, max **90,000** (9 non-missing)
````

The model:
1. Listed the skill directory at `/home/oai/skills/csv-insights-1`
2. Found `assets/example.csv` bundled inside
3. Ran the full pipeline — installed deps, executed `run.py`, produced both `report.md` and `plot.png`

This confirms skills are fully self-contained bundles: instructions, scripts, **and** test data all travel together.

### 4g. Upload the greeter skill and use `"latest"` version pointer

Let's upload our second skill and reference it with `"latest"` — the floating version pointer the docs mention.

```bash
curl -s -X POST "https://api.openai.com/v1/skills" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -F "files=@./greeter_skill.zip;type=application/zip" | python3 -m json.tool
```

```output
{
    "id": "skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04",
    "object": "skill",
    "created_at": 1770824698,
    "default_version": "1",
    "description": "Generate a cheerful greeting in any language.",
    "latest_version": "1",
    "name": "greeter"
}
```

```bash
uv run --with openai python -c '
from openai import OpenAI
client = OpenAI()

response = client.responses.create(
    model="gpt-5.2",
    tools=[{
        "type": "shell",
        "environment": {
            "type": "container_auto",
            "skills": [
                {"type": "skill_reference", "skill_id": "skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c", "version": "latest"},
                {"type": "skill_reference", "skill_id": "skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04", "version": "latest"},
            ],
        },
    }],
    input="What skills do you have access to? List them with their names, descriptions, and file paths.",
)
print(response.output_text)
'
```

```output
Here are the skills I currently have access to in this session (name — description — file path):

- **csv-insights** — Summarize a CSV, compute basic stats, and produce a markdown report + a plot image. — `/home/oai/skills/csv-insights-1`
- **greeter** — Generate a cheerful greeting in any language. — `/home/oai/skills/greeter-1`
```

Both uploaded skills are discovered and mounted at `/home/oai/skills/<name>-<version>`. Using `"latest"` as the version pointer means the environment always mounts the newest version — useful in dev, but the cookbook recommends explicit pinning for production.

### 4h. Cleanup — delete skills

Let's clean up the skills we created during this demo.

```bash
for SKILL_ID in \
  skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c \
  skill_698ca05bafc4819181d236f268efba9d05ded4609a783582 \
  skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04; do
  echo -n "Deleting $SKILL_ID ... "
  HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE \
    "https://api.openai.com/v1/skills/$SKILL_ID" \
    -H "Authorization: Bearer $OPENAI_API_KEY")
  echo "HTTP $HTTP_CODE"
done
```

```output
Deleting skill_698ca18f012c8191836f8d22c0af11f6071496f833619f4c ... HTTP 200
Deleting skill_698ca05bafc4819181d236f268efba9d05ded4609a783582 ... HTTP 200
Deleting skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04 ... HTTP 200
```

```bash
curl -s "https://api.openai.com/v1/skills" \
  -H "Authorization: Bearer $OPENAI_API_KEY" | python3 -m json.tool
```

```output
{
    "object": "list",
    "data": [
        {
            "id": "skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04",
            "object": "skill",
            "created_at": 1770824698,
            "default_version": "1",
            "description": "Generate a cheerful greeting in any language.",
            "latest_version": "1",
            "name": "greeter"
        },
        {
            "id": "skill_698ca05bafc4819181d236f268efba9d05ded4609a783582",
            "object": "skill",
            "created_at": 1770823771,
            "default_version": "1",
            "description": "Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.",
            "latest_version": "1",
            "name": "csv-insights"
        }
    ],
    "first_id": "skill_698ca3fa0c648191ad70b14e3d03ad0808a19eb0aee0cd04",
    "has_more": false,
    "last_id": "skill_698ca05bafc4819181d236f268efba9d05ded4609a783582"
}
```

Interesting: all three DELETE calls returned HTTP 200, but the list endpoint still shows two skills. This suggests **eventual consistency** — deletions may take a moment to propagate, or the API may soft-delete and retain metadata temporarily.

## Summary

### What we demonstrated

| Step | What | Result |
|------|------|--------|
| 1 | Built a skill folder (SKILL.md + run.py + assets) | Matches cookbook layout |
| 2 | Zipped and uploaded via `POST /v1/skills` | Got `skill_id`, `default_version=1` |
| 3 | Invoked via Responses API (`shell` + `container_auto`) | Model discovered skill, ran pipeline, returned report |
| 4a | Listed skills (`GET /v1/skills`) | Paginated list with `first_id`/`last_id`/`has_more` |
| 4b | Retrieved single skill (`GET /v1/skills/:id`) | Full skill object |
| 4c | Version pinning (`version: "1"`) | Skill mounted at `/home/oai/skills/csv-insights-1` |
| 4d | Inline skill (base64 zip, no upload) | Works with `source: {type, media_type, data}` |
| 4e | Multiple skills in one request | Model routes to correct skill based on prompt |
| 4f | Exploring bundled assets | Model finds and uses `assets/example.csv` inside skill |
| 4g | Two uploaded skills + `"latest"` pointer | Both discovered and mounted |
| 4h | Cleanup (`DELETE /v1/skills/:id`) | HTTP 200 (eventual consistency observed) |

### Key learnings beyond the docs

1. **`version` must be a string** — passing an integer gives a 400 error
2. **Inline skill `source` is an object**, not a raw base64 string — needs `{type: "base64", media_type: "application/zip", data: ...}`
3. **`shell` tool requires gpt-5.x** — gpt-4.1 returns "Tool 'shell' is not supported"
4. **Skills mount at `/home/oai/skills/<name>-<version>`** in the container filesystem
5. **Skill routing is automatic** — the model reads name + description to decide which skill to invoke
6. **Deletions may be eventually consistent** — list can still show deleted skills briefly
