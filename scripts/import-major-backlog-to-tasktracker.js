#!/usr/bin/env node

// Import the consolidated major backlog into the Aruvi tasktracker run ledger.
//
// Default mode is a dry run so the grouped item count and payload shape can be
// verified before mutating MCP state. Use --apply to upsert the grouped run and
// MAJ-* rows.

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const DEFAULT_CSV = path.join(ROOT, "docs", "product-roadmap", "major-feature-backlog.csv");
const DEFAULT_RUN_ID = "run-001-major";
const DEFAULT_SOURCE_RUN_ID = "run-001";
const DEFAULT_MCP_URL = "http://127.0.0.1:8787/api/mcp";

const EXPECTED_COLUMNS = [
  "major_id",
  "module",
  "category",
  "service_or_domains",
  "title",
  "priority",
  "release_phases",
  "ship_sizes",
  "pillars",
  "workstreams",
  "source_count",
  "source_id_start",
  "source_id_end",
  "source_ids",
  "implementation_scope",
  "backend_scope",
  "frontend_scope",
  "api_contract",
  "data_model",
  "deterministic_scope",
  "agentic_scope",
  "interaction_model",
  "cost_opportunity",
  "risk_controls",
  "telemetry_contract",
  "test_plan",
  "rollout_guardrail",
  "docs_runbook",
  "acceptance_criteria",
  "dependencies",
  "source_statuses",
  "source_files",
];

function parseArgs(argv) {
  const args = {
    apply: false,
    authorization: process.env.TASKTRACKER_MCP_AUTHORIZATION || "",
    csv: DEFAULT_CSV,
    mcpUrl: process.env.TASKTRACKER_MCP_URL || DEFAULT_MCP_URL,
    printItems: 0,
    runId: DEFAULT_RUN_ID,
    sourceRunId: DEFAULT_SOURCE_RUN_ID,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--apply") {
      args.apply = true;
    } else if (arg === "--authorization") {
      args.authorization = argv[++i];
    } else if (arg === "--csv") {
      args.csv = path.resolve(argv[++i]);
    } else if (arg === "--mcp-url") {
      args.mcpUrl = argv[++i];
    } else if (arg === "--print-items") {
      args.printItems = Number(argv[++i]);
    } else if (arg === "--run-id") {
      args.runId = argv[++i];
    } else if (arg === "--source-run-id") {
      args.sourceRunId = argv[++i];
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}

function parseCsv(text) {
  const rows = [];
  let row = [];
  let value = "";
  let inQuotes = false;

  for (let i = 0; i < text.length; i += 1) {
    const char = text[i];
    const next = text[i + 1];

    if (inQuotes) {
      if (char === '"' && next === '"') {
        value += '"';
        i += 1;
      } else if (char === '"') {
        inQuotes = false;
      } else {
        value += char;
      }
      continue;
    }

    if (char === '"') {
      inQuotes = true;
    } else if (char === ",") {
      row.push(value);
      value = "";
    } else if (char === "\n") {
      row.push(value);
      rows.push(row);
      row = [];
      value = "";
    } else if (char !== "\r") {
      value += char;
    }
  }

  if (value.length > 0 || row.length > 0) {
    row.push(value);
    rows.push(row);
  }

  return rows;
}

function readMajorRows(csvPath) {
  const parsed = parseCsv(fs.readFileSync(csvPath, "utf8"));
  const header = parsed.shift();
  if (header.join(",") !== EXPECTED_COLUMNS.join(",")) {
    throw new Error(`Unexpected major backlog CSV header in ${csvPath}`);
  }

  return parsed
    .filter((row) => row.some((cell) => cell.trim() !== ""))
    .map((row) => {
      const record = {};
      header.forEach((column, index) => {
        record[column] = row[index] || "";
      });
      return record;
    });
}

function splitList(value) {
  return String(value || "")
    .split("|")
    .map((item) => item.trim())
    .filter(Boolean);
}

function slugify(value) {
  return String(value)
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function itemPayload(row, runId, sourceRunId) {
  const sourceIds = splitList(row.source_ids);
  const sourceFiles = splitList(row.source_files);
  const moduleSlug = slugify(row.module);
  const categorySlug = slugify(row.category);

  return {
    runId,
    featureId: row.major_id,
    title: row.title,
    description: row.implementation_scope,
    module: row.module,
    priority: row.priority,
    releasePhase: row.release_phases,
    serviceOrDomain: row.service_or_domains,
    status: "pending",
    conflictZones: [
      `major-module:${moduleSlug}`,
      `major-capability:${moduleSlug}:${categorySlug}`,
    ],
    metadata: {
      source: "major-feature-backlog.csv",
      sourceRunId,
      category: row.category,
      serviceOrDomains: splitList(row.service_or_domains),
      shipSizes: splitList(row.ship_sizes),
      pillars: splitList(row.pillars),
      workstreams: splitList(row.workstreams),
      sourceCount: Number(row.source_count || 0),
      sourceIdStart: row.source_id_start,
      sourceIdEnd: row.source_id_end,
      sourceIds,
      backendScope: row.backend_scope,
      frontendScope: row.frontend_scope,
      apiContract: row.api_contract,
      dataModel: row.data_model,
      deterministicScope: row.deterministic_scope,
      agenticScope: row.agentic_scope,
      interactionModel: row.interaction_model,
      costOpportunity: row.cost_opportunity,
      riskControls: row.risk_controls,
      telemetryContract: row.telemetry_contract,
      testPlan: row.test_plan,
      rolloutGuardrail: row.rollout_guardrail,
      docsRunbook: row.docs_runbook,
      acceptanceCriteria: row.acceptance_criteria,
      dependencies: row.dependencies,
      sourceStatuses: splitList(row.source_statuses),
      sourceFiles,
    },
  };
}

async function rpc(mcpUrl, method, params) {
  const headers = { "Content-Type": "application/json" };
  if (rpc.authorization) {
    headers.Authorization = rpc.authorization;
  }

  const response = await fetch(mcpUrl, {
    method: "POST",
    headers,
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: `${Date.now()}-${Math.random()}`,
      method,
      params,
    }),
  });

  if (!response.ok) {
    throw new Error(`MCP HTTP ${response.status}: ${await response.text()}`);
  }

  const body = await response.json();
  if (body.error) {
    throw new Error(`MCP ${method} failed: ${JSON.stringify(body.error)}`);
  }
  return body.result;
}

async function callTool(mcpUrl, name, args) {
  return rpc(mcpUrl, "tools/call", {
    name,
    arguments: args,
  });
}

async function importRows(args, rows) {
  rpc.authorization = args.authorization;

  await callTool(args.mcpUrl, "agent_work_runs_upsert", {
    id: args.runId,
    status: "active",
    roadmapHash: "major-feature-backlog-v1",
    nextAction: "Claim grouped major backlog items from consolidated CSV",
    metadata: {
      source: path.relative(ROOT, args.csv),
      sourceRunId: args.sourceRunId,
      sourceRows: rows.reduce((total, row) => total + Number(row.source_count || 0), 0),
      majorItems: rows.length,
      shardDirectory: "docs/product-roadmap/major-feature-backlog",
    },
  });

  for (const row of rows) {
    await callTool(args.mcpUrl, "agent_work_items_upsert", itemPayload(row, args.runId, args.sourceRunId));
  }
}

async function main() {
  const args = parseArgs(process.argv);
  const rows = readMajorRows(args.csv);
  const totalSourceRows = rows.reduce(
    (total, row) => total + Number(row.source_count || 0),
    0,
  );

  const summary = {
    mode: args.apply ? "apply" : "dry-run",
    runId: args.runId,
    sourceRunId: args.sourceRunId,
    majorItems: rows.length,
    sourceRows: totalSourceRows,
    modules: new Set(rows.map((row) => row.module)).size,
    csv: path.relative(ROOT, args.csv),
  };

  if (!args.apply) {
    if (args.printItems > 0) {
      console.log(
        JSON.stringify(
          rows
            .slice(0, args.printItems)
            .map((row) => itemPayload(row, args.runId, args.sourceRunId)),
          null,
          2,
        ),
      );
    } else {
      console.log(JSON.stringify(summary, null, 2));
    }
    return;
  }

  await importRows(args, rows);
  console.log(JSON.stringify({ ...summary, status: "imported" }, null, 2));
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
