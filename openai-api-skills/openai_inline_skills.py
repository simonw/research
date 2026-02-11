# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "openai"
# ]
# ///
import io, zipfile, base64
from openai import OpenAI

SKILL_MD = """\
---
name: wc
description: Count words in a file.
---

## Run
python wc.py <file>
"""

WC_PY = """\
import sys
t = open(sys.argv[1]).read()
print(f"Lines: {len(t.splitlines())}  Words: {len(t.split())}  Chars: {len(t)}") 
"""

buf = io.BytesIO()
with zipfile.ZipFile(buf, "w") as zf:
    zf.writestr("s/SKILL.md", SKILL_MD)
    zf.writestr("s/wc.py", WC_PY)

b64_encoded_zip_file = base64.b64encode(buf.getvalue()).decode()

r = OpenAI().responses.create(
    model="gpt-5.2",
    tools=[
        {
            "type": "shell",
            "environment": {
                "type": "container_auto",
                "skills": [
                    {
                        "type": "inline",
                        "name": "wc",
                        "description": "Count words in a file.",
                        "source": {
                            "type": "base64",
                            "media_type": "application/zip",
                            "data": b64_encoded_zip_file,
                        },
                    }
                ],
            },
        }
    ],
    input="Use the wc skill to count words in its own SKILL.md file.",
)
print(r.output_text)

