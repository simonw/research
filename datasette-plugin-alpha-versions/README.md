# Datasette Plugins Analysis - Complete Report

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Overview

This repository contains a comprehensive analysis of all Datasette plugins from the `datasette` organization and `simonw` user on GitHub that start with `datasette-`.

**Analysis Date**: 2025-10-20
**Total Repositories Cloned**: 213 (68 from datasette org, 145 from simonw user)
**Repositories with Notable Features**: 44

## Key Findings

### ALPHA Dependencies
- **39 plugins** depend on Datasette ALPHA versions (1.0a0 through 1.0a19, plus some 0.59a versions)
- **5 plugins** with ALPHA dependencies also have stable releases
- **34 plugins** with ALPHA dependencies have NO stable releases (alpha-only)

### register_permissions() Hook Usage
- **16 plugins** use the `register_permissions()` hook
- **5 plugins** use `register_permissions()` WITHOUT requiring an ALPHA dependency

### Release Status
- **8 plugins** (of the 44 analyzed) have at least one stable release tag
- **36 plugins** have only alpha releases or no releases

## Files in This Repository

### Primary Analysis Files

1. **`datasette_plugins_analysis.json`** ⭐ **MAIN FILE**
   - Complete analysis of all 44 relevant plugins
   - Includes fields:
     - `plugin_name`: Name of the plugin
     - `github_url`: GitHub repository URL
     - `datasette_version`: Datasette version dependency (or "no alpha dependency")
     - `plugin_version`: Current version of the plugin
     - `org`: Organization (datasette or simonw)
     - `uses_register_permissions`: Boolean indicating hook usage
     - `is_alpha`: Boolean indicating ALPHA dependency
     - `alpha_release`: Latest alpha release tag (null if none)
     - `stable_release`: Latest stable release tag (null if none)

### Supporting Files

2. **`alpha_and_permissions.json`**
   - Previous analysis focusing on ALPHA and register_permissions()

3. **`alpha_and_permissions.md`**
   - Markdown report of ALPHA and register_permissions() analysis

4. **`alpha_dependencies.json`**
   - Analysis of only ALPHA dependencies

5. **`alpha_dependencies.md`**
   - Markdown report of ALPHA dependencies

### Analysis Scripts

6. **`analyze_with_releases.py`** ⭐ **FINAL SCRIPT**
   - Comprehensive analysis including git tag information

7. **`analyze_with_permissions.py`**
   - Enhanced analysis including register_permissions() hook

8. **`analyze_alpha_deps.py`**
   - Basic ALPHA dependency analysis

## Notable Plugins

### Plugins with ALPHA Dependencies AND Stable Releases

These 5 plugins currently require ALPHA but have previously had stable releases:

1. **datasette-chronicle** - v0.3 (stable)
2. **datasette-enrichments** - v0.5.1 (stable)
3. **datasette-export-database** - v0.2.1 (stable)
4. **datasette-litestream** - v0.0.3 (stable), requires >=1.0a3
5. **datasette-secrets** - Multiple stable releases, requires no alpha

### Plugins Using register_permissions() Without ALPHA

These 5 plugins use the permissions hook but work with stable Datasette:

1. **datasette-chronicle** (v0.3)
2. **datasette-enrichments** (v0.5)
3. **datasette-export-database** (v0.2)
4. **datasette-secrets** (v0.1a1)
5. **datasette-upload-dbs** (v0.8.1)

## Analysis Methodology

1. **Repository Discovery**: Used GitHub API to fetch all repos starting with `datasette-`
2. **Cloning**: Performed shallow clones (depth=1) of all 213 repositories
3. **Dependency Analysis**: Parsed `setup.py` and `pyproject.toml` for Datasette dependencies
4. **Hook Detection**: Searched Python files for `register_permissions()` function definitions
5. **Release Analysis**: Examined git tags to identify alpha vs stable releases
6. **Version Extraction**: Extracted current plugin versions from package metadata

## Usage

The main analysis file `datasette_plugins_analysis.json` can be used to:

- Identify which plugins need updating when Datasette 1.0 is released
- Find plugins that use the permissions system
- Track which plugins have stable vs alpha-only releases
- Generate upgrade plans for plugin ecosystems

## Statistics Summary

```
Total plugins analyzed:                    44
├── With ALPHA dependencies:               39
│   ├── With stable releases:               5
│   └── Without stable releases:           34
├── Using register_permissions():          16
│   ├── Without ALPHA dependency:           5
│   └── With ALPHA dependency:             11
└── With stable releases:                   8
```

## ALPHA Version Distribution

- `>=1.0a19`: 3 plugins
- `>=1.0a17`: 5 plugins
- `>=1.0a16`: 3 plugins
- `>=1.0a14`: 2 plugins
- `>=1.0a13`: 4 plugins
- `>=1.0a12`: 3 plugins
- Earlier alpha versions: 19 plugins

---

*Generated with Claude Code on 2025-10-20*
