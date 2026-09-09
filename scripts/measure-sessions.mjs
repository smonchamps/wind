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
//   node scripts/measure-sessions.mjs --by-commit <range>   # per increment
//
// Per-increment attribution (kaizen D5 of 2026-09-09): T1 and W3 are
// counted per E-step, each anchored on its closing commit —
// interval(k) = ( t(c(k-1)) , t(c(k)) ]. The first commit of the range is
// E0 by convention (the plan's own commit, closing the design phase), so
// investigation and design never inflate E1. D9 adds the agent share of
// each increment, reported and never capped.
//
// Indicators served: T1 T2 T3 T4 (tokens), P1 (blocked wall > 30 s), M1 (models).

import { readdirSync, createReadStream, existsSync } from "node:fs";
import { basename, join } from "node:path";
import { execFileSync } from "node:child_process";
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
const byCommit = arg("--by-commit", null);

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

export function equiv(u) {
  return (u.input_tokens || 0) + 0.1 * (u.cache_read_input_tokens || 0)
    + 1.25 * (u.cache_creation_input_tokens || 0) + 5 * (u.output_tokens || 0);
}

// The buckets of D5. `commits` are sorted here rather than trusted in
// order: a range read from git is newest-first. Everything at or before
// `from` is outside the job; everything after the last commit is work in
// flight, kept apart in the open bucket so a closed increment is never
// inflated by what came after it.
export function bucketByCommit(events, commits, { from = null } = {}) {
  const sorted = [...commits].sort((a, b) => a.ts - b.ts);
  const fresh = (sha, label, lower, upper) => ({
    sha, label, from: lower, to: upper,
    equiv: 0, agentEquiv: 0, gates: 0, turns: 0,
  });
  const buckets = sorted.map((c, i) =>
    fresh(c.sha, c.label ?? null, i === 0 ? from : sorted[i - 1].ts, c.ts));
  const open = fresh(null, "open", sorted.at(-1)?.ts ?? from, null);

  for (const e of events) {
    const closed = buckets.find(b => (b.from === null || e.ts > b.from) && e.ts <= b.to);
    const last = sorted.at(-1);
    const target = closed ?? (last && e.ts > last.ts ? open : null);
    if (!target) continue; // before the job opened
    if (e.gate) { target.gates++; continue; }
    if (e.agent) target.agentEquiv += e.equiv; else target.equiv += e.equiv;
    target.turns++;
  }
  return { buckets, open };
}

// W3 counts gates PLAYED. gate/SKILL.md mandates one form —
// `powershell ... -File scripts/gate.ps1` — so the script has to be the HEAD
// of a command segment, not merely named somewhere inside one. Measured on
// 2026-09-10, twice: the first rule billed a `grep` of the script as a full
// gate, the second billed the heredoc that WROTE this very test, because the
// invocation appeared as text inside it. A mention is not a run.
export function isGateRun(command) {
  for (const segment of (command || "").split(/&&|\|\||;|\||\n/)) {
    const s = segment.trim().replace(/^\d?>?&?\d?\s*/, "");
    if (/^(?:powershell|pwsh)\b[^'"]*-File\s+\S*gate\.ps1/i.test(s)) return true;
    if (/^['"]?[.\\/]*[\w\\/.-]*gate\.ps1\b/i.test(s)) return true;
  }
  return false;
}

// M3 (D9): reported per increment, never a cap.
export function agentShare(b) {
  const total = b.equiv + b.agentEquiv;
  return total ? b.agentEquiv / total : 0;
}

// T4 as amended by D6: a session at 0 prompts is autonomous work whose
// turns belong to no prompt at all — averaging them onto the other
// sessions' prompts measures the session mix, not the workflow.
export function turnsPerPrompt(sessions) {
  const driven = sessions.filter(s => s.prompts > 0);
  const turns = driven.reduce((a, s) => a + s.turns, 0);
  const prompts = driven.reduce((a, s) => a + s.prompts, 0);
  return {
    ratio: prompts ? turns / prompts : null,
    turns, prompts,
    driven: driven.length,
    excluded: sessions.length - driven.length,
  };
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
        if (u) { s.agentEquiv += equiv(u); if (t) s.events.push({ ts: t, equiv: equiv(u), agent: true }); }
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
          if (t) s.events.push({ ts: t, equiv: equiv(u), agent: false });
        }
        for (const b of e.message.content || []) {
          if (b.type === "tool_use") {
            const command = b.input?.command || "";
            tools.set(b.id, { name: b.name, command, ts: t });
            // W3: one full gate = one gate.ps1 run, wherever it was launched from.
            if (t && isGateRun(command)) s.events.push({ ts: t, equiv: 0, gate: true });
          }
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
    models: {}, slow: {}, contextPerTurn: [], events: [],
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

// Imported by e2e/measure-sessions.test.mjs for the pure functions above;
// the report below runs only when the script is the entry point.
const isMain = process.argv[1] && basename(process.argv[1]) === "measure-sessions.mjs";
if (isMain) {
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
  const t4 = turnsPerPrompt(sessions);
  console.log(`- Sessions / Chief Engineer prompts / turns: ${sessions.length} / ${tot.prompts} / ${tot.turns} (${t4.ratio === null ? "—" : t4.ratio.toFixed(1)} turns/prompt over the ${t4.driven} driven session(s) — target T4 ≤ 25${t4.excluded ? `; ${t4.excluded} session(s) at 0 prompts excluded, D6` : ""})`);
  console.log(`- Input equiv., main thread: ${fmtM(totEquiv)}; agents: ${tot.nAgents} transcripts, ${fmtM(tot.agentEquiv)} (${((tot.agentEquiv / (totEquiv + tot.agentEquiv)) * 100 || 0).toFixed(1)} %)`);
  console.log(`- Average context re-read per turn: ${fmtK(ctxGlobal)} (target T2 ≤ 200 k)`);
  console.log(`- Sessions > 24 h of wall: ${marathons.length}${marathons.length ? " (" + marathons.map(s => s.id.slice(0, 8)).join(", ") + ")" : ""} (target T3: 0)`);
  console.log(`- Main-thread models: ${fmtModels(tot.models)}`);
  console.log(`- Agent models: ${fmtModels(tot.agentModels)} (target M1: exploration lowered)`);
  console.log(`- Blocked wall on commands > 30 s: ${fmtMin(totBlockedWall)} (target P1 ≤ 15 min / 2 weeks) — ${Object.entries(tot.slow).map(([c, l]) => `${c}: ${l.n} (total ${fmtMin(l.totalMs)}, max ${Math.round(l.maxMs / 1000)} s)`).join("; ") || "—"}`);

  if (byCommit) {
    const log = execFileSync("git", ["log", "--format=%H|%cI|%s", byCommit], { encoding: "utf8" });
    const commits = log.trim().split("\n").filter(Boolean).map(line => {
      const [sha, iso, ...subject] = line.split("|");
      return { sha, ts: new Date(iso), label: subject.join("|").slice(0, 44) };
    });
    const { buckets, open } = bucketByCommit(sessions.flatMap(s => s.events), commits, { from: since });

    console.log(`\n## Per increment (D5) — range ${byCommit}, ${commits.length} commit(s)`);
    console.log("# The FIRST row is E0 by convention: the plan's own commit, closing");
    console.log("# the design phase, so investigation never inflates E1.\n");
    console.log("| Increment | Commit | Main thread | Agents | Agent share | Gates | Turns |");
    console.log("|---|---|---|---|---|---|---|");
    for (const [n, b] of buckets.entries()) {
      const share = agentShare(b);
      const flag = share > 0.4 ? " ⚠" : "";
      const name = n === 0 ? "E0 (design)" : `E${n}`;
      console.log(`| ${name} — ${b.label} | \`${b.sha.slice(0, 7)}\` | ${fmtM(b.equiv)} | ${fmtM(b.agentEquiv)} | ${(share * 100).toFixed(1)} %${flag} | ${b.gates} | ${b.turns} |`);
    }
    if (open.turns || open.gates) {
      console.log(`| *(in flight)* | — | ${fmtM(open.equiv)} | ${fmtM(open.agentEquiv)} | ${(agentShare(open) * 100).toFixed(1)} % | ${open.gates} | ${open.turns} |`);
    }
    const over = buckets.filter(b => agentShare(b) > 0.4);
    if (over.length) {
      console.log(`\n⚠ M3 (D9): ${over.length} increment(s) past ~40 % agent share — their plan entry must state what the agents FOUND: ${over.map(b => b.sha.slice(0, 7)).join(", ")}.`);
    }
    console.log("# T1 = main thread + agents per row; W3 = the Gates column.");
  }

}
