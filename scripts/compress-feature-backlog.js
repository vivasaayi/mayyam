#!/usr/bin/env node

// Compress detailed roadmap rows into larger implementation packages.
//
// The detailed backlog intentionally explodes every module/category/service
// across pillars and workstreams. This script preserves traceability while
// creating larger delivery items that are easier to claim, implement, and
// validate as batches.

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const ROADMAP_DIR = path.join(ROOT, "docs", "product-roadmap");
const OUT_CSV = path.join(ROADMAP_DIR, "major-feature-backlog.csv");
const OUT_MD = path.join(ROADMAP_DIR, "major-feature-backlog.md");
const MAX_SOURCE_ROWS_PER_MAJOR_ITEM = 100;

const INPUT_COLUMNS = [
  "id",
  "module",
  "category",
  "service_or_domain",
  "pillar",
  "workstream",
  "feature",
  "release_phase",
  "ship_size",
  "user_story",
  "ship_slice",
  "implementation_detail",
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
  "priority",
  "dependency",
  "source_status",
];

const OUTPUT_COLUMNS = [
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

function listBacklogFiles() {
  return fs
    .readdirSync(ROADMAP_DIR, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(ROADMAP_DIR, entry.name, "feature-backlog.csv"))
    .filter((file) => fs.existsSync(file))
    .sort();
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

function readBacklog(file) {
  const parsed = parseCsv(fs.readFileSync(file, "utf8"));
  const header = parsed.shift();
  if (header.join(",") !== INPUT_COLUMNS.join(",")) {
    throw new Error(`Unexpected CSV header in ${file}`);
  }

  return parsed
    .filter((row) => row.some((cell) => cell.trim() !== ""))
    .map((row) => {
      const record = {};
      header.forEach((column, index) => {
        record[column] = row[index] || "";
      });
      record.source_file = path.relative(ROOT, file);
      return record;
    });
}

function csvEscape(value) {
  const raw = String(value ?? "");
  if (/[",\n\r]/.test(raw)) {
    return `"${raw.replace(/"/g, '""')}"`;
  }
  return raw;
}

function csvLine(values) {
  return values.map(csvEscape).join(",");
}

function unique(values) {
  return [...new Set(values.filter((value) => value && value.trim() !== ""))].sort();
}

function summarize(values, maxItems = 8) {
  const items = unique(values);
  if (items.length <= maxItems) {
    return items.join(" | ");
  }
  return `${items.slice(0, maxItems).join(" | ")} | +${items.length - maxItems} more`;
}

function highestPriority(priorities) {
  const order = ["P0", "P1", "P2", "P3"];
  const present = new Set(priorities);
  return order.find((priority) => present.has(priority)) || summarize(priorities, 1);
}

function serviceTitle(services) {
  if (services.length <= 2) {
    return services.join(" + ");
  }
  return `${services[0]} + ${services.length - 1} related services`;
}

function naturalGroupKey(row) {
  return [row.module, row.category, row.service_or_domain].join("\u001f");
}

const SET_FIELDS = [
  "pillar",
  "workstream",
  "release_phase",
  "priority",
  "ship_size",
  "backend_scope",
  "frontend_scope",
  "api_contract",
  "data_model",
  "telemetry_contract",
  "test_plan",
  "rollout_guardrail",
  "docs_runbook",
  "acceptance_criteria",
  "dependency",
  "source_status",
  "source_file",
];

const FIRST_TEXT_FIELDS = [
  "deterministic_scope",
  "agentic_scope",
  "interaction_model",
  "cost_opportunity",
  "risk_controls",
];

function createAggregate(row) {
  const sets = Object.fromEntries(SET_FIELDS.map((field) => [field, new Set()]));
  const firstTexts = Object.fromEntries(FIRST_TEXT_FIELDS.map((field) => [field, ""]));

  return {
    module: row.module,
    category: row.category,
    service_or_domain: row.service_or_domain,
    source_count: 0,
    source_ids: [],
    sets,
    firstTexts,
  };
}

function addRowToAggregate(group, row) {
  group.source_count += 1;
  group.source_ids.push(row.id);

  for (const field of SET_FIELDS) {
    const value = row[field];
    if (value && value.trim() !== "") {
      group.sets[field].add(value);
    }
  }

  for (const field of FIRST_TEXT_FIELDS) {
    if (!group.firstTexts[field] && row[field] && row[field].trim() !== "") {
      group.firstTexts[field] = row[field];
    }
  }
}

function readNaturalGroups(files) {
  const groupsByKey = new Map();
  let sourceRowCount = 0;

  for (const file of files) {
    for (const row of readBacklog(file)) {
      sourceRowCount += 1;
      const key = naturalGroupKey(row);
      if (!groupsByKey.has(key)) {
        groupsByKey.set(key, createAggregate(row));
      }
      addRowToAggregate(groupsByKey.get(key), row);
    }
  }

  const naturalGroups = [...groupsByKey.values()].sort((a, b) => {
    return (
      a.module.localeCompare(b.module) ||
      a.category.localeCompare(b.category) ||
      a.service_or_domain.localeCompare(b.service_or_domain)
    );
  });

  return { sourceRowCount, naturalGroups };
}

function packGroups(naturalGroups) {
  const packs = [];
  let current = null;

  for (const group of naturalGroups) {
    const boundary = `${group.module}\u001f${group.category}`;
    const canJoin =
      current &&
      current.boundary === boundary &&
      current.source_count + group.source_count <= MAX_SOURCE_ROWS_PER_MAJOR_ITEM;

    if (!canJoin) {
      current = {
        boundary,
        module: group.module,
        category: group.category,
        groups: [],
        source_count: 0,
      };
      packs.push(current);
    }

    current.groups.push(group);
    current.source_count += group.source_count;
  }

  return packs;
}

function valuesFromSet(groups, field) {
  return groups.flatMap((group) => [...group.sets[field]]);
}

function firstText(groups, field) {
  return groups.find((group) => group.firstTexts[field])?.firstTexts[field] || "";
}

function majorRow(pack, index) {
  const services = unique(pack.groups.map((group) => group.service_or_domain));
  const sourceIds = pack.groups.flatMap((group) => group.source_ids);
  const pillars = unique(valuesFromSet(pack.groups, "pillar"));
  const workstreams = unique(valuesFromSet(pack.groups, "workstream"));
  const releasePhases = unique(valuesFromSet(pack.groups, "release_phase"));
  const priorities = unique(valuesFromSet(pack.groups, "priority"));
  const title = `${pack.module}: ${pack.category} / ${serviceTitle(services)}`;

  return {
    major_id: `MAJ-${String(index + 1).padStart(4, "0")}`,
    module: pack.module,
    category: pack.category,
    service_or_domains: services.join(" | "),
    title,
    priority: highestPriority(priorities),
    release_phases: releasePhases.join(" | "),
    ship_sizes: unique(valuesFromSet(pack.groups, "ship_size")).join(" | "),
    pillars: pillars.join(" | "),
    workstreams: workstreams.join(" | "),
    source_count: pack.source_count,
    source_id_start: sourceIds[0],
    source_id_end: sourceIds[sourceIds.length - 1],
    source_ids: sourceIds.join(" | "),
    implementation_scope: `Deliver the ${pack.category} capability for ${serviceTitle(
      services,
    )} across ${pillars.length} pillar(s) and ${workstreams.length} workstream(s), preserving the source acceptance criteria from ${pack.source_count} detailed backlog row(s).`,
    backend_scope: summarize(valuesFromSet(pack.groups, "backend_scope")),
    frontend_scope: summarize(valuesFromSet(pack.groups, "frontend_scope")),
    api_contract: summarize(valuesFromSet(pack.groups, "api_contract")),
    data_model: summarize(valuesFromSet(pack.groups, "data_model")),
    deterministic_scope: firstText(pack.groups, "deterministic_scope"),
    agentic_scope: firstText(pack.groups, "agentic_scope"),
    interaction_model: firstText(pack.groups, "interaction_model"),
    cost_opportunity: firstText(pack.groups, "cost_opportunity"),
    risk_controls: firstText(pack.groups, "risk_controls"),
    telemetry_contract: summarize(valuesFromSet(pack.groups, "telemetry_contract")),
    test_plan: summarize(valuesFromSet(pack.groups, "test_plan")),
    rollout_guardrail: summarize(valuesFromSet(pack.groups, "rollout_guardrail")),
    docs_runbook: summarize(valuesFromSet(pack.groups, "docs_runbook")),
    acceptance_criteria: summarize(valuesFromSet(pack.groups, "acceptance_criteria"), 4),
    dependencies: summarize(valuesFromSet(pack.groups, "dependency")),
    source_statuses: unique(valuesFromSet(pack.groups, "source_status")).join(" | "),
    source_files: unique(valuesFromSet(pack.groups, "source_file")).join(" | "),
  };
}

function writeMajorBacklog(majorRows) {
  const lines = [
    csvLine(OUTPUT_COLUMNS),
    ...majorRows.map((row) => csvLine(OUTPUT_COLUMNS.map((column) => row[column]))),
  ];
  fs.writeFileSync(OUT_CSV, `${lines.join("\n")}\n`);
}

function writeSummary(sourceRowCount, naturalGroups, majorRows) {
  const rowsByModule = new Map();
  for (const row of majorRows) {
    rowsByModule.set(row.module, (rowsByModule.get(row.module) || 0) + 1);
  }

  const moduleLines = [...rowsByModule.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([module, count]) => `| ${module} | ${count} |`)
    .join("\n");

  const markdown = `# Major Feature Backlog

This file compresses the generated implementation backlog into larger delivery units for roadmap execution.

## Compression Rule

- Source rows: ${sourceRowCount.toLocaleString()} from \`docs/product-roadmap/*/feature-backlog.csv\`.
- Natural grouping: \`module + category + service_or_domain\`, producing ${naturalGroups.length.toLocaleString()} service capability groups.
- Packing rule: adjacent service capability groups are combined only within the same \`module + category\` boundary, up to ${MAX_SOURCE_ROWS_PER_MAJOR_ITEM} source rows per major item.
- Output: ${majorRows.length.toLocaleString()} major feature items in \`major-feature-backlog.csv\`.
- Traceability: every major row carries \`source_ids\`, \`source_id_start\`, \`source_id_end\`, and \`source_files\`.

## Why This Shape

The original backlog is useful as a detailed acceptance matrix, but it is too granular for implementation claims. The major backlog keeps related work together so one claim can deliver telemetry, posture, triage, interaction, tests, rollout, and runbook coverage as a coherent capability.

## Major Items By Module

| Module | Major items |
| --- | ---: |
${moduleLines}

## Regeneration

Run:

\`\`\`bash
node scripts/compress-feature-backlog.js
\`\`\`
`;

  fs.writeFileSync(OUT_MD, markdown);
}

function main() {
  const files = listBacklogFiles();
  const { sourceRowCount, naturalGroups } = readNaturalGroups(files);
  const packs = packGroups(naturalGroups);
  const majorRows = packs.map(majorRow);

  writeMajorBacklog(majorRows);
  writeSummary(sourceRowCount, naturalGroups, majorRows);

  console.log(
    `Compressed ${sourceRowCount} source rows into ${majorRows.length} major items.`,
  );
  console.log(`Wrote ${path.relative(ROOT, OUT_CSV)}`);
  console.log(`Wrote ${path.relative(ROOT, OUT_MD)}`);
}

main();
