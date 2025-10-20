#!/usr/bin/env python3
"""
Comprehensive analysis of datasette plugins including:
1. ALPHA version dependencies
2. register_permissions() usage
3. Git release tags (alpha vs stable)
"""
import os
import json
import re
import subprocess
from pathlib import Path
import tomli


def extract_version_from_setup_py(setup_path):
    """Extract version from setup.py"""
    try:
        with open(setup_path, 'r') as f:
            content = f.read()
        version_match = re.search(r'version\s*=\s*["\']([^"\']+)["\']', content)
        if version_match:
            return version_match.group(1)
    except Exception as e:
        pass
    return None


def extract_version_from_pyproject(pyproject_path):
    """Extract version from pyproject.toml"""
    try:
        with open(pyproject_path, 'rb') as f:
            data = tomli.load(f)
        if 'project' in data and 'version' in data['project']:
            return data['project']['version']
        elif 'tool' in data and 'poetry' in data['tool'] and 'version' in data['tool']['poetry']:
            return data['tool']['poetry']['version']
    except Exception as e:
        pass
    return None


def check_datasette_dependency(file_path, file_type):
    """Check if file contains datasette alpha dependency"""
    try:
        if file_type == 'setup.py':
            with open(file_path, 'r') as f:
                content = f.read()
            alpha_patterns = [
                r'["\']datasette[>=<~!]+([^"\']*a[^"\']*)["\']',
                r'["\']datasette\s*>=?\s*([^"\']*a[^"\']*)["\']',
            ]
            for pattern in alpha_patterns:
                matches = re.findall(pattern, content)
                if matches:
                    for match in matches:
                        if 'a' in match:
                            return match.strip()

        elif file_type == 'pyproject.toml':
            with open(file_path, 'rb') as f:
                data = tomli.load(f)
            dependencies = []
            if 'project' in data and 'dependencies' in data['project']:
                dependencies.extend(data['project']['dependencies'])
            if 'tool' in data and 'poetry' in data['tool'] and 'dependencies' in data['tool']['poetry']:
                poetry_deps = data['tool']['poetry']['dependencies']
                if isinstance(poetry_deps, dict):
                    for dep, constraint in poetry_deps.items():
                        if dep.lower() == 'datasette':
                            if isinstance(constraint, str):
                                dependencies.append(f"datasette{constraint}")
                            elif isinstance(constraint, dict) and 'version' in constraint:
                                dependencies.append(f"datasette{constraint['version']}")
                else:
                    dependencies.extend(poetry_deps)
            for dep in dependencies:
                if isinstance(dep, str) and 'datasette' in dep.lower():
                    match = re.search(r'datasette\s*([>=<~!]+.*)', dep, re.IGNORECASE)
                    if match:
                        version_constraint = match.group(1)
                        if 'a' in version_constraint:
                            return version_constraint.strip()
    except Exception as e:
        pass
    return None


def check_register_permissions(repo_path):
    """Check if repository uses register_permissions() hook"""
    try:
        for py_file in repo_path.rglob('*.py'):
            try:
                with open(py_file, 'r') as f:
                    content = f.read()
                    if 'register_permissions' in content:
                        if re.search(r'def\s+register_permissions\s*\(', content):
                            return True
            except Exception:
                pass
    except Exception:
        pass
    return False


def get_git_tags(repo_path):
    """Get all git tags from a repository and categorize them"""
    try:
        result = subprocess.run(
            ['git', 'tag', '-l'],
            cwd=repo_path,
            capture_output=True,
            text=True,
            timeout=10
        )

        if result.returncode != 0:
            return None, None

        tags = [tag.strip() for tag in result.stdout.strip().split('\n') if tag.strip()]

        if not tags:
            return None, None

        alpha_tags = []
        stable_tags = []

        for tag in tags:
            # Check if tag contains alpha indicator (a, alpha, etc.)
            # Common patterns: 0.1a0, 0.1-alpha, 0.1.a1, etc.
            if re.search(r'a\d+|alpha|beta|rc|dev', tag, re.IGNORECASE):
                alpha_tags.append(tag)
            else:
                # Assume it's a stable release if it doesn't have pre-release indicators
                stable_tags.append(tag)

        # Get the most recent of each type (last in list, as git tag -l lists chronologically by default)
        latest_alpha = alpha_tags[-1] if alpha_tags else None
        latest_stable = stable_tags[-1] if stable_tags else None

        return latest_alpha, latest_stable

    except Exception as e:
        return None, None


def analyze_repo(repo_path, org_name, repo_name):
    """Analyze a single repository"""
    setup_py = repo_path / 'setup.py'
    pyproject_toml = repo_path / 'pyproject.toml'

    alpha_version = None
    plugin_version = None
    uses_register_permissions = check_register_permissions(repo_path)

    # Check setup.py first
    if setup_py.exists():
        alpha_version = check_datasette_dependency(setup_py, 'setup.py')
        if not plugin_version:
            plugin_version = extract_version_from_setup_py(setup_py)

    # Check pyproject.toml
    if pyproject_toml.exists():
        if not alpha_version:
            alpha_version = check_datasette_dependency(pyproject_toml, 'pyproject.toml')
        if not plugin_version:
            plugin_version = extract_version_from_pyproject(pyproject_toml)

    # Get git release information
    latest_alpha_release, latest_stable_release = get_git_tags(repo_path)

    github_url = f"https://github.com/{org_name}/{repo_name}"

    # Return data if it has alpha dependency OR uses register_permissions
    if alpha_version or uses_register_permissions:
        return {
            'plugin_name': repo_name,
            'github_url': github_url,
            'datasette_version': alpha_version or 'no alpha dependency',
            'plugin_version': plugin_version or 'unknown',
            'org': org_name,
            'uses_register_permissions': uses_register_permissions,
            'is_alpha': alpha_version is not None,
            'alpha_release': latest_alpha_release,
            'stable_release': latest_stable_release
        }

    return None


def main():
    base_dir = Path('/home/user/private-scratch')

    results = []

    print("Analyzing datasette org repositories...")
    datasette_org_dir = base_dir / 'datasette-org'
    if datasette_org_dir.exists():
        repos = sorted(datasette_org_dir.iterdir())
        for i, repo_dir in enumerate(repos, 1):
            if repo_dir.is_dir():
                print(f"  [{i}/{len(repos)}] {repo_dir.name}")
                result = analyze_repo(repo_dir, 'datasette', repo_dir.name)
                if result:
                    results.append(result)

    print("\nAnalyzing simonw repositories...")
    simonw_dir = base_dir / 'simonw-user'
    if simonw_dir.exists():
        repos = sorted(simonw_dir.iterdir())
        for i, repo_dir in enumerate(repos, 1):
            if repo_dir.is_dir():
                print(f"  [{i}/{len(repos)}] {repo_dir.name}")
                result = analyze_repo(repo_dir, 'simonw', repo_dir.name)
                if result:
                    results.append(result)

    # Save comprehensive results as JSON
    output_file = base_dir / 'datasette_plugins_analysis.json'
    with open(output_file, 'w') as f:
        json.dump(results, f, indent=2)

    # Generate statistics
    alpha_count = sum(1 for r in results if r['is_alpha'])
    register_perms_count = sum(1 for r in results if r['uses_register_permissions'])
    has_stable_release = sum(1 for r in results if r['stable_release'] is not None)
    alpha_dep_with_stable = sum(1 for r in results if r['is_alpha'] and r['stable_release'] is not None)
    alpha_dep_no_stable = sum(1 for r in results if r['is_alpha'] and r['stable_release'] is None)

    print(f"\n{'='*60}")
    print("Analysis complete!")
    print(f"{'='*60}")
    print(f"Total plugins analyzed: {len(results)}")
    print(f"Plugins with ALPHA dependencies: {alpha_count}")
    print(f"Plugins using register_permissions(): {register_perms_count}")
    print(f"Plugins with at least one stable release: {has_stable_release}")
    print(f"ALPHA dependency plugins WITH stable release: {alpha_dep_with_stable}")
    print(f"ALPHA dependency plugins WITHOUT stable release: {alpha_dep_no_stable}")
    print(f"\nResults saved to: {output_file}")


if __name__ == '__main__':
    main()
