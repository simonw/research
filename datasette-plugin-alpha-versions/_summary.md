Datasette Plugins Analysis presents a systematic evaluation of 44 key plugins from the Datasette ecosystem, focusing on dependencies, permissions hooks, and release patterns as of October 2025. The study finds that 89% of these plugins rely on ALPHA versions of Datasette, with only 8 plugins having stable releases and just 5 supporting stable Datasette while using advanced hooks like `register_permissions()`. The open datasets, such as [`datasette_plugins_analysis.json`](https://github.com/simonw/datasette-plugins-analysis/blob/main/datasette_plugins_analysis.json) and analysis scripts, support deeper inspection and maintenance planning as Datasette nears its 1.0 milestone. This enables maintainers to prioritize updates for plugins with alpha dependencies and track release maturity across the ecosystem.

**Key Findings:**
- 39 plugins depend on Datasette ALPHA versions; 34 of these have no stable releases.
- Only 5 plugins use `register_permissions()` without requiring ALPHA Datasette.
- 8 of the analyzed plugins currently offer at least one stable release.  
- Main analysis and scripts are available [here](https://github.com/simonw/datasette-plugins-analysis) for further plugin and dependency tracking.
