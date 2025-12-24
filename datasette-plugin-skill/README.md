# Datasette Plugin Writer Skill

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A comprehensive skill for writing Datasette plugins, covering all aspects of plugin development from initial setup through publishing.

## What This Skill Covers

- **Quick Start**: Using the cookiecutter template to bootstrap a new plugin
- **Plugin Hooks**: Comprehensive reference for the most commonly used hooks including:
  - `prepare_connection` - Custom SQL functions
  - `register_routes` - Custom URL endpoints
  - `render_cell` - Custom table cell rendering
  - `extra_template_vars` - Template context augmentation
  - `table_actions` - Custom table menu items
  - `actor_from_request` - Authentication
  - `permission_allowed` - Authorization

- **Request/Response Objects**: Complete API reference for handling HTTP requests and creating responses
- **Database API**: How to query and modify SQLite databases from plugins
- **Plugin Configuration**: Reading and using configuration from datasette.yaml
- **Static Assets & Templates**: Adding CSS, JavaScript, and custom HTML templates
- **Common Patterns**: Ready-to-use code examples for frequent plugin tasks
- **Testing**: How to write tests for your plugin
- **Publishing**: Deploying to GitHub and PyPI

## When to Use This Skill

Invoke this skill when:
- Creating a new Datasette plugin from scratch
- Adding functionality to Datasette through the plugin system
- Understanding how to use specific plugin hooks
- Learning about the request/response or database APIs
- Setting up authentication or permissions
- Adding custom routes or output formats

## Key Features

- Includes the exact command to use with cookiecutter via uvx
- Provides working code examples for each major plugin hook
- Documents the complete request and response object APIs
- Covers both sync and async plugin hooks
- Explains URL design patterns and best practices
- Shows how to test plugins effectively

## Examples of What You Can Build

With this skill, you can create plugins that:
- Add custom SQL functions (e.g., datasette-haversine for geospatial queries)
- Create custom visualizations (e.g., datasette-cluster-map for maps)
- Add authentication systems (e.g., datasette-auth-passwords)
- Export data in custom formats (e.g., datasette-atom for Atom feeds)
- Modify how data is rendered (e.g., datasette-render-markdown)
- Add admin interfaces and custom pages

## Resources

This skill was created using documentation and examples from:
- [Datasette documentation](https://docs.datasette.io/)
- [Plugin hooks reference](https://docs.datasette.io/en/stable/plugin_hooks.html)
- [datasette-plugin cookiecutter template](https://github.com/simonw/datasette-plugin)
- Various example plugins from the [Datasette plugins directory](https://datasette.io/plugins)
