#!/usr/bin/env node

/**
 * OpenResponses Compliance Test CLI
 *
 * A command-line tool for running compliance tests against implementations
 * of the OpenResponses API standard.
 */

import { Command } from "commander";
import chalk from "chalk";
import {
  runAllTests,
  listTests,
  type TestConfig,
  type TestResult,
} from "./compliance-tests.js";

const program = new Command();

// ============================================================================
// Output Formatting
// ============================================================================

function formatStatus(status: TestResult["status"]): string {
  switch (status) {
    case "passed":
      return chalk.green("PASS");
    case "failed":
      return chalk.red("FAIL");
    case "running":
      return chalk.yellow("RUNNING");
    case "skipped":
      return chalk.gray("SKIP");
    case "pending":
      return chalk.gray("PENDING");
  }
}

function formatDuration(ms?: number): string {
  if (ms === undefined) return "";
  if (ms < 1000) return chalk.gray(`${ms}ms`);
  return chalk.gray(`${(ms / 1000).toFixed(2)}s`);
}

function printTestResult(result: TestResult, verbose: boolean): void {
  const status = formatStatus(result.status);
  const duration = formatDuration(result.duration);
  const streamInfo =
    result.streamEvents !== undefined
      ? chalk.cyan(` [${result.streamEvents} events]`)
      : "";

  console.log(`  ${status} ${result.name} ${duration}${streamInfo}`);

  if (result.status === "failed" && result.errors && result.errors.length > 0) {
    for (const error of result.errors) {
      console.log(`       ${chalk.red("✗")} ${error}`);
    }
  }

  if (verbose && result.status !== "running") {
    if (result.request) {
      console.log(chalk.gray(`       Request: ${JSON.stringify(result.request, null, 2).split("\n").join("\n       ")}`));
    }
    if (result.response && result.status === "failed") {
      const responseStr = typeof result.response === "string"
        ? result.response
        : JSON.stringify(result.response, null, 2);
      console.log(chalk.gray(`       Response: ${responseStr.split("\n").join("\n       ")}`));
    }
  }
}

function printSummary(results: TestResult[]): void {
  const passed = results.filter((r) => r.status === "passed").length;
  const failed = results.filter((r) => r.status === "failed").length;
  const skipped = results.filter((r) => r.status === "skipped").length;
  const total = results.length;

  console.log();
  console.log(chalk.bold("Summary:"));
  console.log(
    `  ${chalk.green(`${passed} passed`)}, ${chalk.red(`${failed} failed`)}${
      skipped > 0 ? `, ${chalk.gray(`${skipped} skipped`)}` : ""
    } (${total} total)`
  );

  const totalDuration = results.reduce((sum, r) => sum + (r.duration || 0), 0);
  console.log(`  Total time: ${formatDuration(totalDuration)}`);
}

function printResultsJson(results: TestResult[]): void {
  const output = {
    summary: {
      total: results.length,
      passed: results.filter((r) => r.status === "passed").length,
      failed: results.filter((r) => r.status === "failed").length,
      skipped: results.filter((r) => r.status === "skipped").length,
    },
    results: results.map((r) => ({
      id: r.id,
      name: r.name,
      description: r.description,
      status: r.status,
      duration: r.duration,
      errors: r.errors,
      streamEvents: r.streamEvents,
      request: r.request,
      response: r.response,
    })),
  };
  console.log(JSON.stringify(output, null, 2));
}

// ============================================================================
// Commands
// ============================================================================

program
  .name("openresponses-compliance")
  .description("CLI tool for running OpenResponses API compliance tests")
  .version("1.0.0");

program
  .command("run")
  .description("Run compliance tests against a server")
  .requiredOption("-u, --url <url>", "Base URL of the API server")
  .requiredOption("-k, --api-key <key>", "API key for authentication")
  .option("-m, --model <model>", "Model name to use for tests", "gpt-4o-mini")
  .option(
    "-H, --auth-header <header>",
    "Name of the authorization header",
    "Authorization"
  )
  .option("--no-bearer", "Don't use 'Bearer ' prefix for API key")
  .option(
    "-t, --timeout <ms>",
    "Request timeout in milliseconds",
    (v) => parseInt(v, 10),
    60000
  )
  .option("-f, --filter <tests...>", "Run only specific test IDs")
  .option("-j, --json", "Output results as JSON")
  .option("-v, --verbose", "Show detailed request/response information")
  .action(async (options) => {
    const config: TestConfig = {
      baseUrl: options.url.replace(/\/$/, ""), // Remove trailing slash
      apiKey: options.apiKey,
      authHeaderName: options.authHeader,
      useBearerPrefix: options.bearer !== false,
      model: options.model,
      timeout: options.timeout,
    };

    if (!options.json) {
      console.log();
      console.log(chalk.bold("OpenResponses Compliance Test Runner"));
      console.log(chalk.gray(`Server: ${config.baseUrl}`));
      console.log(chalk.gray(`Model: ${config.model}`));
      console.log();
    }

    const results: TestResult[] = [];
    const completedIds = new Set<string>();

    try {
      await runAllTests(
        config,
        (result) => {
          if (!options.json) {
            // Only print once per test (skip "running" if we haven't seen it)
            if (result.status !== "running" || !completedIds.has(result.id)) {
              if (result.status !== "running") {
                completedIds.add(result.id);
              }
              printTestResult(result, options.verbose);
            }
          }
          if (result.status !== "running" && result.status !== "pending") {
            results.push(result);
          }
        },
        options.filter
      );

      if (options.json) {
        printResultsJson(results);
      } else {
        printSummary(results);
      }

      const hasFailures = results.some((r) => r.status === "failed");
      process.exit(hasFailures ? 1 : 0);
    } catch (error) {
      if (!options.json) {
        console.error(chalk.red(`\nError: ${error instanceof Error ? error.message : error}`));
      } else {
        console.log(JSON.stringify({ error: String(error) }));
      }
      process.exit(1);
    }
  });

program
  .command("list")
  .description("List available compliance tests")
  .option("-j, --json", "Output as JSON")
  .action((options) => {
    const tests = listTests();

    if (options.json) {
      console.log(JSON.stringify(tests, null, 2));
    } else {
      console.log();
      console.log(chalk.bold("Available Compliance Tests:"));
      console.log();
      for (const test of tests) {
        console.log(`  ${chalk.cyan(test.id)}`);
        console.log(`    ${test.name}`);
        console.log(`    ${chalk.gray(test.description)}`);
        console.log();
      }
    }
  });

// Parse arguments
program.parse();
