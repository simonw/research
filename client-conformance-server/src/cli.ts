#!/usr/bin/env node

/**
 * OpenResponses Client Conformance Server CLI
 *
 * A mock server for testing client implementations of the OpenResponses API.
 */

import { Command } from "commander";
import chalk from "chalk";
import { startServer, testScenarios, type ServerConfig } from "./server.js";

const program = new Command();

program
  .name("openresponses-conformance-server")
  .description("Mock server for testing OpenResponses API client implementations")
  .version("1.0.0");

program
  .command("start")
  .description("Start the conformance test server")
  .option("-p, --port <port>", "Port to listen on", (v) => parseInt(v, 10), 8080)
  .option("-h, --host <host>", "Host to bind to", "127.0.0.1")
  .option("-v, --verbose", "Enable verbose logging")
  .action(async (options) => {
    const config: ServerConfig = {
      port: options.port,
      host: options.host,
      verbose: options.verbose || false,
    };

    console.log();
    console.log(chalk.bold("Starting OpenResponses Client Conformance Server..."));
    console.log();

    await startServer(config);

    console.log(chalk.gray("Press Ctrl+C to stop the server"));
    console.log();
  });

program
  .command("scenarios")
  .description("List available test scenarios")
  .option("-j, --json", "Output as JSON")
  .action((options) => {
    if (options.json) {
      console.log(JSON.stringify(testScenarios, null, 2));
    } else {
      console.log();
      console.log(chalk.bold("Available Test Scenarios:"));
      console.log();

      for (const scenario of testScenarios) {
        console.log(`  ${chalk.cyan(scenario.id)}`);
        console.log(`    ${scenario.name}`);
        console.log(`    ${chalk.gray(scenario.description)}`);
        console.log(`    ${chalk.yellow("Trigger:")} ${scenario.trigger}`);
        console.log();
      }
    }
  });

program
  .command("test-request")
  .description("Send a test request to a running server")
  .option("-u, --url <url>", "Server URL", "http://127.0.0.1:8080")
  .option("-s, --scenario <scenario>", "Test scenario to trigger")
  .option("--stream", "Request streaming response")
  .action(async (options) => {
    const url = options.url.replace(/\/$/, "");

    const requestBody: Record<string, unknown> = {
      model: options.scenario ? `test-${options.scenario}` : "test-model",
      input: [
        {
          type: "message",
          role: "user",
          content: "Hello, this is a test request.",
        },
      ],
    };

    if (options.stream) {
      requestBody.stream = true;
    }

    // Add tools for tool-call scenarios
    if (options.scenario?.includes("tool")) {
      requestBody.tools = [
        {
          type: "function",
          name: "get_weather",
          description: "Get weather for a location",
          parameters: {
            type: "object",
            properties: {
              location: { type: "string" },
            },
            required: ["location"],
          },
        },
      ];
    }

    console.log();
    console.log(chalk.bold("Sending test request..."));
    console.log(chalk.gray(`URL: ${url}/responses`));
    console.log(chalk.gray(`Request: ${JSON.stringify(requestBody, null, 2)}`));
    console.log();

    try {
      const response = await fetch(`${url}/responses`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(requestBody),
      });

      const contentType = response.headers.get("content-type") || "";

      if (contentType.includes("text/event-stream")) {
        console.log(chalk.green("Streaming response:"));
        const reader = response.body?.getReader();
        const decoder = new TextDecoder();

        if (reader) {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            process.stdout.write(decoder.decode(value, { stream: true }));
          }
        }
      } else {
        const data = await response.json();
        console.log(chalk.green(`Status: ${response.status}`));
        console.log(chalk.gray(JSON.stringify(data, null, 2)));
      }
    } catch (error) {
      console.error(chalk.red(`Error: ${error instanceof Error ? error.message : error}`));
      process.exit(1);
    }
  });

program.parse();
