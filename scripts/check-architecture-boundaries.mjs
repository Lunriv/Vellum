#!/usr/bin/env node
/**
 * Architecture Boundary Guard for Vellum (Section 16 & Section 19/20 of ARCHITECTURE.md).
 * Runs in CI and locally without requiring Rust/Node builds to pass first.
 */

import fs from "node:fs";
import path from "node:path";

const repoRoot = process.cwd();
const violations = [];

function walkFiles(dir, exts) {
  if (!fs.existsSync(dir)) return [];
  const results = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "target" || entry.name === "dist") {
        continue;
      }
      results.push(...walkFiles(full, exts));
    } else if (exts.some((ext) => entry.name.endsWith(ext))) {
      results.push(full);
    }
  }
  return results;
}

function checkForbiddenPatterns(files, rules) {
  for (const file of files) {
    const rel = path.relative(repoRoot, file).replace(/\\/g, "/");
    const content = fs.readFileSync(file, "utf8");
    const lines = content.split(/\r?\n/);
    for (const rule of rules) {
      if (rule.skipTestFiles && (rel.includes("/tests/") || rel.endsWith("_test.rs") || rel.endsWith(".test.ts") || rel.endsWith(".test.tsx"))) {
        continue;
      }
      lines.forEach((line, idx) => {
        if (rule.pattern.test(line)) {
          violations.push({
            file: rel,
            line: idx + 1,
            rule: rule.id,
            message: rule.message,
            snippet: line.trim(),
          });
        }
      });
    }
  }
}

// 1. Kernel must stay pure & deterministic: no Axum, SQLx, Reqwest, HTTP/LLM/runtime/api/persistence imports.
const kernelDir = path.join(repoRoot, "apps", "server", "src", "kernel");
const kernelFiles = walkFiles(kernelDir, [".rs"]);
checkForbiddenPatterns(kernelFiles, [
  {
    id: "KERNEL_NO_INFRA_CRATES",
    pattern: /^\s*(?:pub\s+)?use\s+(?:axum|sqlx|reqwest|tokio::net|crate::(?:api|persistence|models|narration|stream|runtime))\b/,
    message: "Kernel must not depend on Axum, SQLx, Reqwest, LLM adapters, API, Persistence, Stream, Narration, or Runtime.",
  },
  {
    id: "KERNEL_NO_ASYNC_IO_CLIENT",
    pattern: /\b(?:reqwest::Client|sqlx::SqlitePool|axum::Router)\b/,
    message: "Kernel must remain pure deterministic logic without HTTP or database clients.",
  },
]);

// 2. Narration must not directly reference SQLite pools or mutate authoritative persistence repositories.
const narrationDir = path.join(repoRoot, "apps", "server", "src", "narration");
const narrationFiles = walkFiles(narrationDir, [".rs"]);
checkForbiddenPatterns(narrationFiles, [
  {
    id: "NARRATION_NO_DIRECT_DB_WRITE",
    pattern: /^\s*(?:pub\s+)?use\s+(?:sqlx|crate::persistence)\b/,
    message: "Narration layer must not directly access persistence; only TurnOrchestrator persists accepted narration.",
  },
]);

// 3. Frontend DVP security guardrails: no arbitrary HTML injection or dynamic code execution.
const webSrcDir = path.join(repoRoot, "apps", "web", "src");
const webFiles = walkFiles(webSrcDir, [".ts", ".tsx", ".js", ".jsx"]);
checkForbiddenPatterns(webFiles, [
  {
    id: "WEB_NO_DANGEROUS_HTML",
    pattern: /\bdangerouslySetInnerHTML\b/,
    message: "DVP components must render through typed React components; dangerouslySetInnerHTML is forbidden.",
  },
  {
    id: "WEB_NO_DYNAMIC_EVAL",
    pattern: /\b(?:eval\s*\(|new\s+Function\s*\()/,
    message: "Dynamic code execution is forbidden in the Vellum frontend.",
    skipTestFiles: true,
  },
]);

// 4. V1 Forbidden Stack Guard in Cargo.toml & package.json (Section 19 #9)
const rootManifests = [
  path.join(repoRoot, "Cargo.toml"),
  path.join(repoRoot, "apps", "server", "Cargo.toml"),
  path.join(repoRoot, "package.json"),
  path.join(repoRoot, "apps", "web", "package.json"),
].filter((p) => fs.existsSync(p));

checkForbiddenPatterns(rootManifests, [
  {
    id: "NO_FORBIDDEN_V1_INFRA",
    pattern: /["'\s](?:redis|deadpool-redis|neo4rs|langgraph|celery|faiss)["'\s=]/i,
    message: "Section 19 #9 forbids introducing Redis, Neo4j, LangGraph, Celery, or FAISS in V1.",
  },
]);

if (violations.length > 0) {
  console.error(`Architecture boundary check FAILED with ${violations.length} violation(s):\n`);
  for (const v of violations) {
    console.error(`- [${v.rule}] ${v.file}:${v.line}\n  ${v.message}\n  > ${v.snippet}`);
  }
  process.exit(1);
}

console.log(
  `Architecture boundary check passed (scanned ${kernelFiles.length} kernel, ${narrationFiles.length} narration, ${webFiles.length} web files, ${rootManifests.length} manifests).`
);
