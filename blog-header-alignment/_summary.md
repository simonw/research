Addressing a subtle header alignment issue on simonwillison.net, this investigation tracked down a persistent ~1px height mismatch between left and right headers caused by anchor elements generating taller inline boxes than plain text due to font metrics. Multiple fixes—including removing position:relative/top:1px hacks and setting explicit heights—proved fragile. The optimal solution was applying display:flex and align-items:center to the h2.overband headers, normalizing their height regardless of link presence and enabling precise vertical alignment. Padding-top was also adjusted to shift header contents down by user-requested 1–3px. For reproducible testing, the Showboat tool was used for screenshot capture and stepwise CSS live editing ([Showboat](https://github.com/simonw/showboat)).

Key findings:
- Inline anchor tags, even with identical font and line-height, produce taller boxes than plain text, leading to subtle alignment mismatches.
- Legacy position/top hacks provide only imprecise visual compensation and do not solve underlying flow issues.
- Setting display:flex and align-items:center on shared parent header elements reliably normalizes vertical sizing.
- All attempts with explicit heights proved rigid, breaking with font/zoom changes.
- Final CSS is robust across viewport sizes and scaling—see local test screenshots for proof.
