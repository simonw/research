#!/usr/bin/env python3
"""
Analyze datasette plugins to find:
1. Which ones depend on ALPHA versions
2. Which non-alpha plugins use register_permissions() hook
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


def check_register_permissions(repo_path):
    """Check if repository uses register_permissions() hook"""
    try:
        # Search for register_permissions in all .py files
        for py_file in repo_path.rglob('*.py'):
            try:
                with open(py_file, 'r') as f:
                    content = f.read()
                    if 'register_permissions' in content:
                        # Check if it's actually defining the hook, not just importing
                        if re.search(r'def\s+register_permissions\s*\(', content):
                            return True
            except Exception:
                pass
    except Exception:
        pass
    return False


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
            'is_alpha': alpha_version is not None
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
    with open(base_dir / 'alpha_and_permissions.json', 'w') as f:
        json.dump(results, f, indent=2)

    # Generate markdown report
    with open(base_dir / 'alpha_and_permissions.md', 'w') as f:
        alpha_count = sum(1 for r in results if r['is_alpha'])
        register_perms_count = sum(1 for r in results if r['uses_register_permissions'])
        register_perms_no_alpha = sum(1 for r in results if r['uses_register_permissions'] and not r['is_alpha'])

        f.write('# Datasette Plugins Analysis\n\n')
        f.write(f'- **Total plugins with ALPHA dependencies**: {alpha_count}\n')
        f.write(f'- **Total plugins using register_permissions()**: {register_perms_count}\n')
        f.write(f'- **Plugins using register_permissions() WITHOUT alpha dependency**: {register_perms_no_alpha}\n\n')

        # Alpha dependencies section
        f.write('## Plugins with ALPHA Dependencies\n\n')
        for result in results:
            if result['is_alpha']:
                f.write(f"### {result['plugin_name']}\n\n")
                f.write(f"- **GitHub URL**: {result['github_url']}\n")
                f.write(f"- **Plugin Version**: {result['plugin_version']}\n")
                f.write(f"- **Datasette Version**: {result['datasette_version']}\n")
                f.write(f"- **Organization**: {result['org']}\n")
                f.write(f"- **Uses register_permissions()**: {'Yes' if result['uses_register_permissions'] else 'No'}\n\n")

        # Register permissions without alpha section
        f.write('## Plugins using register_permissions() WITHOUT ALPHA dependency\n\n')
        for result in results:
            if result['uses_register_permissions'] and not result['is_alpha']:
                f.write(f"### {result['plugin_name']}\n\n")
                f.write(f"- **GitHub URL**: {result['github_url']}\n")
                f.write(f"- **Plugin Version**: {result['plugin_version']}\n")
                f.write(f"- **Datasette Version**: {result['datasette_version']}\n")
                f.write(f"- **Organization**: {result['org']}\n\n")

    print(f"Analysis complete!")
    print(f"- Plugins with ALPHA dependencies: {alpha_count}")
    print(f"- Plugins using register_permissions(): {register_perms_count}")
    print(f"- Plugins using register_permissions() WITHOUT alpha: {register_perms_no_alpha}")
    print(f"\nResults saved to:")
    print(f"  - {base_dir / 'alpha_and_permissions.json'}")
    print(f"  - {base_dir / 'alpha_and_permissions.md'}")


if __name__ == '__main__':
    main()
