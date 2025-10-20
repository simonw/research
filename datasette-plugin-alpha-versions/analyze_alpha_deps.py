#!/usr/bin/env python3
"""
Analyze datasette plugins to find which ones depend on ALPHA versions
"""
import os
import json
import re
from pathlib import Path
import tomli


def extract_version_from_setup_py(setup_path):
    """Extract version from setup.py"""
    try:
        with open(setup_path, 'r') as f:
            content = f.read()

        # Look for version= in setup()
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

        # Check different locations for version
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

            # Look for datasette in install_requires or extras_require
            # Match patterns like: "datasette>=1.0a0", "datasette>=1.0a1", etc.
            alpha_patterns = [
                r'["\']datasette[>=<~!]+([^"\']*a[^"\']*)["\']',
                r'["\']datasette\s*>=?\s*([^"\']*a[^"\']*)["\']',
            ]

            for pattern in alpha_patterns:
                matches = re.findall(pattern, content)
                if matches:
                    # Extract the version constraint
                    for match in matches:
                        if 'a' in match:  # alpha version
                            return match.strip()

        elif file_type == 'pyproject.toml':
            with open(file_path, 'rb') as f:
                data = tomli.load(f)

            # Check dependencies in different locations
            dependencies = []

            # project.dependencies
            if 'project' in data and 'dependencies' in data['project']:
                dependencies.extend(data['project']['dependencies'])

            # tool.poetry.dependencies
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

            # Check each dependency for alpha version
            for dep in dependencies:
                if isinstance(dep, str) and 'datasette' in dep.lower():
                    # Extract version constraint
                    match = re.search(r'datasette\s*([>=<~!]+.*)', dep, re.IGNORECASE)
                    if match:
                        version_constraint = match.group(1)
                        if 'a' in version_constraint:  # alpha version
                            return version_constraint.strip()

    except Exception as e:
        pass

    return None


def analyze_repo(repo_path, org_name, repo_name):
    """Analyze a single repository"""
    setup_py = repo_path / 'setup.py'
    pyproject_toml = repo_path / 'pyproject.toml'

    alpha_version = None
    plugin_version = None

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

    if alpha_version:
        github_url = f"https://github.com/{org_name}/{repo_name}"
        return {
            'plugin_name': repo_name,
            'github_url': github_url,
            'datasette_version': alpha_version,
            'plugin_version': plugin_version or 'unknown',
            'org': org_name
        }

    return None


def main():
    base_dir = Path('/home/user/private-scratch')

    results = []

    # Analyze datasette org repos
    datasette_org_dir = base_dir / 'datasette-org'
    if datasette_org_dir.exists():
        for repo_dir in sorted(datasette_org_dir.iterdir()):
            if repo_dir.is_dir():
                result = analyze_repo(repo_dir, 'datasette', repo_dir.name)
                if result:
                    results.append(result)

    # Analyze simonw repos
    simonw_dir = base_dir / 'simonw-user'
    if simonw_dir.exists():
        for repo_dir in sorted(simonw_dir.iterdir()):
            if repo_dir.is_dir():
                result = analyze_repo(repo_dir, 'simonw', repo_dir.name)
                if result:
                    results.append(result)

    # Save results as JSON
    with open(base_dir / 'alpha_dependencies.json', 'w') as f:
        json.dump(results, f, indent=2)

    # Generate markdown report
    with open(base_dir / 'alpha_dependencies.md', 'w') as f:
        f.write('# Datasette Plugins with ALPHA Dependencies\n\n')
        f.write(f'Found {len(results)} plugins depending on Datasette ALPHA versions.\n\n')

        for result in results:
            f.write(f"## {result['plugin_name']}\n\n")
            f.write(f"- **GitHub URL**: {result['github_url']}\n")
            f.write(f"- **Plugin Version**: {result['plugin_version']}\n")
            f.write(f"- **Datasette Version**: {result['datasette_version']}\n")
            f.write(f"- **Organization**: {result['org']}\n\n")

    print(f"Analysis complete! Found {len(results)} plugins with ALPHA dependencies.")
    print(f"Results saved to:")
    print(f"  - {base_dir / 'alpha_dependencies.json'}")
    print(f"  - {base_dir / 'alpha_dependencies.md'}")

    # Print summary
    if results:
        print("\nPlugins with ALPHA dependencies:")
        for result in results:
            print(f"  - {result['plugin_name']}: {result['datasette_version']}")


if __name__ == '__main__':
    main()
