#!/usr/bin/env node
// measure-sessions.mjs — measures the Claude Code usage on Wind (kaizen, decision D2).
//
// Reads the local transcripts under ~/.claude/projects/<project key>/ (a
// machine-specific path, like install-workstation.ps1) and prints, per
// session and in aggregate: tokens (input equivalents: cacheRead ×0.1,
// cacheCreate ×1.25, output ×5), Chief Engineer prompts, assistant turns, average
// context re-read per turn, wall hours, commands > 30 s per category
// (duration = timestamp of the result − timestamp of the turn that
// launches the tool; background commands return immediately and are
// therefore not counted), share of the subagents
// (<session>/subagents/*.jsonl).
//
// Usage:
//   node scripts/measure-sessions.mjs                        # last 7 days
//   node scripts/measure-sessions.mjs --since 2026-08-11 --until 2026-08-23
//
// Indicators served: T1 T2 T3 T4 (tokens), P1 (blocked wall > 30 s), M1 (models).

import { readdirSync, createReadStream, existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import { createInterface } from "node:readline";

const args = process.argv.slice(2);
function arg(name, fallback) {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
}
const until = new Date(arg("--until", new Date().toISOString().slice(0, 10)) + "T23:59:59Z");
const sinceDefault = new Date(until.getTime() - 6 * 86400_000).toISOString().slice(0, 10);
const since = new Date(arg("--since", sinceDefault) + "T00:00:00Z");

// The project key is the cwd, separators and ':' replaced by '-'.
const key = process.cwd().replace(/[\\/:.]/g, "-");
const folder = join(homedir(), ".claude", "projects", key);

const THRESHOLD_MS = 30_000;

function category(name, command) {
  const c = (command || "").toLowerCase();
  if (/git push/.test(c)) return "push";
  if (/gh run (watch|list|view)/.test(c)) return "ci";
  if (/npm test|playwright|node e2e|[\\/]e2e[\\/ ]/.test(c)) return "e2e";
  if (/cargo (build|test|clippy|run)/.test(c)) return "cargo";
  return name === "Bash" || name === "PowerShell" ? "shell" : "tool";
}

function equiv(u) {
  return (u.input_tokens || 0) + 0.1 * (u.cache_read_input_tokens || 0)
    + 1.25 * (u.cache_creation_input_tokens || 0) + 5 * (u.output_tokens || 0);
}

async function readFile(path, s, sidechain) {
  const tools = new Map(); // tool_use id -> { name, command, ts } for duration and category
  const rl = createInterface({ input: createReadStream(path, "utf8"), crlfDelay: Infinity });
  for await (const line of rl) {
    let e;
    try { e = JSON.parse(line); } catch { continue; }
    const t = e.timestamp ? new Date(e.timestamp) : null;
    if (t && !sidechain) {
      if (!s.start || t < s.start) s.start = t;
      if (!s.end || t > s.end) s.end = t;
    }
    if (e.type === "assistant" && e.message) {
      const u = e.message.usage;
      if (sidechain || e.isSidechain) {
        if (u) s.agentEquiv += equiv(u);
        const m = e.message.model || "?";
        s.agentModels[m] = (s.agentModels[m] || 0) + 1;
      } else {
        s.turns++;
        const m = e.message.model || "?";
        s.models[m] = (s.models[m] || 0) + 1;
        if (u) {
          s.input += u.input_tokens || 0;
          s.cacheRead += u.cache_read_input_tokens || 0;
          s.cacheCreate += u.cache_creation_input_tokens || 0;
          s.output += u.output_tokens || 0;
          s.contextPerTurn.push((u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0));
        }
        for (const b of e.message.content || []) {
          if (b.type === "tool_use") tools.set(b.id, { name: b.name, command: b.input?.command || "", ts: t });
        }
      }
    } else if (e.type === "user" && !sidechain && !e.isSidechain && e.message) {
      const c = e.message.content;
      const text = typeof c === "string" ? c
        : Array.isArray(c) && !c.some(b => b.type === "tool_result")
          ? (c.find(b => b.type === "text")?.text ?? null) : null;
      // A Chief Engineer prompt is a text message that is neither meta nor a machine
      // message (command invocation, local output: content in a <tag>).
      if (text !== null && !e.isMeta && !text.trimStart().startsWith("<")) s.prompts++;
      const id = Array.isArray(c) ? c.find(b => b.type === "tool_result")?.tool_use_id : null;
      const o = id && tools.get(id);
      // Only the shell tools count as blocked wall: waiting on an
      // AskUserQuestion or an agent is not a foreground command.
      if (o && t && o.ts && (o.name === "Bash" || o.name === "PowerShell")) {
        const d = t - o.ts;
        if (d > THRESHOLD_MS) {
          const cat = category(o.name, o.command);
          const l = (s.slow[cat] ||= { n: 0, totalMs: 0, maxMs: 0 });
          l.n++; l.totalMs += d; l.maxMs = Math.max(l.maxMs, d);
        }
      }
    }
  }
}

async function readSession(file) {
  const s = {
    id: file.replace(".jsonl", ""),
    start: null, end: null, prompts: 0, turns: 0,
    input: 0, cacheRead: 0, cacheCreate: 0, output: 0,
    agentEquiv: 0, nAgents: 0, agentModels: {},
    models: {}, slow: {}, contextPerTurn: [],
  };
  await readFile(join(folder, file), s, false);
  const subagents = join(folder, s.id, "subagents");
  if (existsSync(subagents)) {
    for (const f of readdirSync(subagents).filter(f => f.endsWith(".jsonl"))) {
      s.nAgents++;
      await readFile(join(subagents, f), s, true);
    }
  }
  return s;
}

const M = 1_000_000;
const fmtM = n => (n / M).toFixed(1) + " M";
const fmtK = n => Math.round(n / 1000) + " k";
const fmtH = ms => (ms / 3600_000).toFixed(1) + " h";
const fmtMin = ms => Math.round(ms / 60_000) + " min";

const files = readdirSync(folder).filter(f => f.endsWith(".jsonl"));
const sessions = [];
for (const f of files) {
  const s = await readSession(f);
  if (!s.start || s.end < since || s.start > until) continue;
  sessions.push(s);
}
sessions.sort((a, b) => a.start - b.start);

console.log(`# Session measurement — ${since.toISOString().slice(0, 10)} → ${until.toISOString().slice(0, 10)}`);
console.log(`# Folder: ${folder} (${sessions.length} sessions in the window)\n`);

console.log("| Session | Start | Wall | Prompts | Turns | Input equiv. | Avg ctx/turn | Agents | Blocked wall > 30 s |");
console.log("|---|---|---|---|---|---|---|---|---|");
const tot = { prompts: 0, turns: 0, agentEquiv: 0, nAgents: 0, models: {}, agentModels: {}, slow: {} };
let totEquiv = 0, totBlockedWall = 0, totCtx = [];
for (const s of sessions) {
  const eq = equiv({ input_tokens: s.input, cache_read_input_tokens: s.cacheRead, cache_creation_input_tokens: s.cacheCreate, output_tokens: s.output });
  const avgCtx = s.contextPerTurn.length ? s.contextPerTurn.reduce((a, b) => a + b, 0) / s.contextPerTurn.length : 0;
  const blockedWall = Object.values(s.slow).reduce((a, l) => a + l.totalMs, 0);
  const slowTxt = Object.entries(s.slow).map(([c, l]) => `${c}:${l.n}×(${fmtMin(l.totalMs)})`).join(" ") || "—";
  console.log(`| ${s.id.slice(0, 8)} | ${s.start.toISOString().slice(0, 16).replace("T", " ")} | ${fmtH(s.end - s.start)} | ${s.prompts} | ${s.turns} | ${fmtM(eq)} | ${fmtK(avgCtx)} | ${s.nAgents} (${fmtM(s.agentEquiv)}) | ${slowTxt} |`);
  tot.prompts += s.prompts; tot.turns += s.turns; tot.agentEquiv += s.agentEquiv; tot.nAgents += s.nAgents;
  totEquiv += eq; totBlockedWall += blockedWall; totCtx.push(...s.contextPerTurn);
  for (const [m, n] of Object.entries(s.models)) tot.models[m] = (tot.models[m] || 0) + n;
  for (const [m, n] of Object.entries(s.agentModels)) tot.agentModels[m] = (tot.agentModels[m] || 0) + n;
  for (const [c, l] of Object.entries(s.slow)) {
    const g = (tot.slow[c] ||= { n: 0, totalMs: 0, maxMs: 0 });
    g.n += l.n; g.totalMs += l.totalMs; g.maxMs = Math.max(g.maxMs, l.maxMs);
  }
}

const ctxGlobal = totCtx.length ? totCtx.reduce((a, b) => a + b, 0) / totCtx.length : 0;
const marathons = sessions.filter(s => s.end - s.start > 24 * 3600_000);
const fmtModels = o => Object.entries(o).sort((a, b) => b[1] - a[1]).map(([m, n]) => `${m}: ${n}`).join(", ") || "—";
console.log(`\n## Aggregate`);
console.log(`- Sessions / Chief Engineer prompts / turns: ${sessions.length} / ${tot.prompts} / ${tot.turns} (${tot.prompts ? (tot.turns / tot.prompts).toFixed(1) : "—"} turns/prompt — target T4 ≤ 25)`);
console.log(`- Input equiv., main thread: ${fmtM(totEquiv)}; agents: ${tot.nAgents} transcripts, ${fmtM(tot.agentEquiv)} (${((tot.agentEquiv / (totEquiv + tot.agentEquiv)) * 100 || 0).toFixed(1)} %)`);
console.log(`- Average context re-read per turn: ${fmtK(ctxGlobal)} (target T2 ≤ 200 k)`);
console.log(`- Sessions > 24 h of wall: ${marathons.length}${marathons.length ? " (" + marathons.map(s => s.id.slice(0, 8)).join(", ") + ")" : ""} (target T3: 0)`);
console.log(`- Main-thread models: ${fmtModels(tot.models)}`);
console.log(`- Agent models: ${fmtModels(tot.agentModels)} (target M1: exploration lowered)`);
console.log(`- Blocked wall on commands > 30 s: ${fmtMin(totBlockedWall)} (target P1 ≤ 15 min / 2 weeks) — ${Object.entries(tot.slow).map(([c, l]) => `${c}: ${l.n} (total ${fmtMin(l.totalMs)}, max ${Math.round(l.maxMs / 1000)} s)`).join("; ") || "—"}`);
