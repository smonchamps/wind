#!/usr/bin/env node
// mesurer-sessions.mjs — mesure l'usage Claude Code sur Wind (kaizen, décision D2).
//
// Lit les transcripts locaux sous ~/.claude/projects/<clé projet>/ (chemin
// propre à la machine, comme installer-poste.ps1) et sort, par session et en
// agrégat : tokens (équivalents input : cacheRead ×0,1, cacheCreate ×1,25,
// output ×5), prompts CE, tours assistant, contexte moyen relu par tour,
// heures de mur, commandes > 30 s par catégorie (durée = horodatage du
// résultat − horodatage du tour qui lance l'outil ; les commandes en
// arrière-plan rendent la main tout de suite et ne comptent donc pas),
// part des sous-agents (<session>/subagents/*.jsonl).
//
// Usage :
//   node scripts/mesurer-sessions.mjs                        # 7 derniers jours
//   node scripts/mesurer-sessions.mjs --depuis 2026-08-11 --jusqua 2026-08-23
//
// Indicateurs servis : T1 T2 T3 T4 (tokens), P1 (mur bloqué > 30 s), M1 (modèles).

import { readdirSync, createReadStream, existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import { createInterface } from "node:readline";

const args = process.argv.slice(2);
function arg(nom, defaut) {
  const i = args.indexOf(nom);
  return i >= 0 && args[i + 1] ? args[i + 1] : defaut;
}
const jusqua = new Date(arg("--jusqua", new Date().toISOString().slice(0, 10)) + "T23:59:59Z");
const depuisDefaut = new Date(jusqua.getTime() - 6 * 86400_000).toISOString().slice(0, 10);
const depuis = new Date(arg("--depuis", depuisDefaut) + "T00:00:00Z");

// La clé projet est le cwd, séparateurs et ':' remplacés par '-'.
const cle = process.cwd().replace(/[\\/:.]/g, "-");
const dossier = join(homedir(), ".claude", "projects", cle);

const SEUIL_MS = 30_000;

function categorie(nom, commande) {
  const c = (commande || "").toLowerCase();
  if (/git push/.test(c)) return "push";
  if (/gh run (watch|list|view)/.test(c)) return "ci";
  if (/npm test|playwright|node e2e|[\\/]e2e[\\/ ]/.test(c)) return "e2e";
  if (/cargo (build|test|clippy|run)/.test(c)) return "cargo";
  return nom === "Bash" || nom === "PowerShell" ? "shell" : "outil";
}

function equiv(u) {
  return (u.input_tokens || 0) + 0.1 * (u.cache_read_input_tokens || 0)
    + 1.25 * (u.cache_creation_input_tokens || 0) + 5 * (u.output_tokens || 0);
}

async function lireFichier(chemin, s, sidechain) {
  const outils = new Map(); // tool_use id -> { nom, commande, ts } pour durée et catégorie
  const rl = createInterface({ input: createReadStream(chemin, "utf8"), crlfDelay: Infinity });
  for await (const ligne of rl) {
    let e;
    try { e = JSON.parse(ligne); } catch { continue; }
    const t = e.timestamp ? new Date(e.timestamp) : null;
    if (t && !sidechain) {
      if (!s.debut || t < s.debut) s.debut = t;
      if (!s.fin || t > s.fin) s.fin = t;
    }
    if (e.type === "assistant" && e.message) {
      const u = e.message.usage;
      if (sidechain || e.isSidechain) {
        if (u) s.agentEquiv += equiv(u);
        const m = e.message.model || "?";
        s.agentModeles[m] = (s.agentModeles[m] || 0) + 1;
      } else {
        s.tours++;
        const m = e.message.model || "?";
        s.modeles[m] = (s.modeles[m] || 0) + 1;
        if (u) {
          s.input += u.input_tokens || 0;
          s.cacheRead += u.cache_read_input_tokens || 0;
          s.cacheCreate += u.cache_creation_input_tokens || 0;
          s.output += u.output_tokens || 0;
          s.contexteParTour.push((u.input_tokens || 0) + (u.cache_read_input_tokens || 0) + (u.cache_creation_input_tokens || 0));
        }
        for (const b of e.message.content || []) {
          if (b.type === "tool_use") outils.set(b.id, { nom: b.name, commande: b.input?.command || "", ts: t });
        }
      }
    } else if (e.type === "user" && !sidechain && !e.isSidechain && e.message) {
      const c = e.message.content;
      const texte = typeof c === "string" ? c
        : Array.isArray(c) && !c.some(b => b.type === "tool_result")
          ? (c.find(b => b.type === "text")?.text ?? null) : null;
      // Un prompt CE est un message texte qui n'est ni méta ni un message
      // machine (invocation de commande, sortie locale : contenu en <balise>).
      if (texte !== null && !e.isMeta && !texte.trimStart().startsWith("<")) s.prompts++;
      const id = Array.isArray(c) ? c.find(b => b.type === "tool_result")?.tool_use_id : null;
      const o = id && outils.get(id);
      // Seuls les outils shell comptent en mur bloqué : une attente
      // d'AskUserQuestion ou d'un agent n'est pas une commande au premier plan.
      if (o && t && o.ts && (o.nom === "Bash" || o.nom === "PowerShell")) {
        const d = t - o.ts;
        if (d > SEUIL_MS) {
          const cat = categorie(o.nom, o.commande);
          const l = (s.lentes[cat] ||= { n: 0, totalMs: 0, maxMs: 0 });
          l.n++; l.totalMs += d; l.maxMs = Math.max(l.maxMs, d);
        }
      }
    }
  }
}

async function lireSession(fichier) {
  const s = {
    id: fichier.replace(".jsonl", ""),
    debut: null, fin: null, prompts: 0, tours: 0,
    input: 0, cacheRead: 0, cacheCreate: 0, output: 0,
    agentEquiv: 0, nAgents: 0, agentModeles: {},
    modeles: {}, lentes: {}, contexteParTour: [],
  };
  await lireFichier(join(dossier, fichier), s, false);
  const sousAgents = join(dossier, s.id, "subagents");
  if (existsSync(sousAgents)) {
    for (const f of readdirSync(sousAgents).filter(f => f.endsWith(".jsonl"))) {
      s.nAgents++;
      await lireFichier(join(sousAgents, f), s, true);
    }
  }
  return s;
}

const M = 1_000_000;
const fmtM = n => (n / M).toFixed(1) + " M";
const fmtK = n => Math.round(n / 1000) + " k";
const fmtH = ms => (ms / 3600_000).toFixed(1) + " h";
const fmtMin = ms => Math.round(ms / 60_000) + " min";

const fichiers = readdirSync(dossier).filter(f => f.endsWith(".jsonl"));
const sessions = [];
for (const f of fichiers) {
  const s = await lireSession(f);
  if (!s.debut || s.fin < depuis || s.debut > jusqua) continue;
  sessions.push(s);
}
sessions.sort((a, b) => a.debut - b.debut);

console.log(`# Mesure des sessions — ${depuis.toISOString().slice(0, 10)} → ${jusqua.toISOString().slice(0, 10)}`);
console.log(`# Dossier : ${dossier} (${sessions.length} sessions dans la fenêtre)\n`);

console.log("| Session | Début | Mur | Prompts | Tours | Équiv. input | Ctx moyen/tour | Agents | Mur bloqué > 30 s |");
console.log("|---|---|---|---|---|---|---|---|---|");
const tot = { prompts: 0, tours: 0, agentEquiv: 0, nAgents: 0, modeles: {}, agentModeles: {}, lentes: {} };
let totEquiv = 0, totMurBloque = 0, totCtx = [];
for (const s of sessions) {
  const eq = equiv({ input_tokens: s.input, cache_read_input_tokens: s.cacheRead, cache_creation_input_tokens: s.cacheCreate, output_tokens: s.output });
  const ctxMoyen = s.contexteParTour.length ? s.contexteParTour.reduce((a, b) => a + b, 0) / s.contexteParTour.length : 0;
  const murBloque = Object.values(s.lentes).reduce((a, l) => a + l.totalMs, 0);
  const lentesTxt = Object.entries(s.lentes).map(([c, l]) => `${c}:${l.n}×(${fmtMin(l.totalMs)})`).join(" ") || "—";
  console.log(`| ${s.id.slice(0, 8)} | ${s.debut.toISOString().slice(0, 16).replace("T", " ")} | ${fmtH(s.fin - s.debut)} | ${s.prompts} | ${s.tours} | ${fmtM(eq)} | ${fmtK(ctxMoyen)} | ${s.nAgents} (${fmtM(s.agentEquiv)}) | ${lentesTxt} |`);
  tot.prompts += s.prompts; tot.tours += s.tours; tot.agentEquiv += s.agentEquiv; tot.nAgents += s.nAgents;
  totEquiv += eq; totMurBloque += murBloque; totCtx.push(...s.contexteParTour);
  for (const [m, n] of Object.entries(s.modeles)) tot.modeles[m] = (tot.modeles[m] || 0) + n;
  for (const [m, n] of Object.entries(s.agentModeles)) tot.agentModeles[m] = (tot.agentModeles[m] || 0) + n;
  for (const [c, l] of Object.entries(s.lentes)) {
    const g = (tot.lentes[c] ||= { n: 0, totalMs: 0, maxMs: 0 });
    g.n += l.n; g.totalMs += l.totalMs; g.maxMs = Math.max(g.maxMs, l.maxMs);
  }
}

const ctxGlobal = totCtx.length ? totCtx.reduce((a, b) => a + b, 0) / totCtx.length : 0;
const marathons = sessions.filter(s => s.fin - s.debut > 24 * 3600_000);
const fmtModeles = o => Object.entries(o).sort((a, b) => b[1] - a[1]).map(([m, n]) => `${m}: ${n}`).join(", ") || "—";
console.log(`\n## Agrégat`);
console.log(`- Sessions / prompts CE / tours : ${sessions.length} / ${tot.prompts} / ${tot.tours} (${tot.prompts ? (tot.tours / tot.prompts).toFixed(1) : "—"} tours/prompt — cible T4 ≤ 25)`);
console.log(`- Équiv. input fil principal : ${fmtM(totEquiv)} ; agents : ${tot.nAgents} transcripts, ${fmtM(tot.agentEquiv)} (${((tot.agentEquiv / (totEquiv + tot.agentEquiv)) * 100 || 0).toFixed(1)} %)`);
console.log(`- Contexte moyen relu par tour : ${fmtK(ctxGlobal)} (cible T2 ≤ 200 k)`);
console.log(`- Sessions > 24 h de mur : ${marathons.length}${marathons.length ? " (" + marathons.map(s => s.id.slice(0, 8)).join(", ") + ")" : ""} (cible T3 : 0)`);
console.log(`- Modèles fil principal : ${fmtModeles(tot.modeles)}`);
console.log(`- Modèles agents : ${fmtModeles(tot.agentModeles)} (cible M1 : exploration abaissée)`);
console.log(`- Mur bloqué sur commandes > 30 s : ${fmtMin(totMurBloque)} (cible P1 ≤ 15 min / 2 sem.) — ${Object.entries(tot.lentes).map(([c, l]) => `${c}: ${l.n} (total ${fmtMin(l.totalMs)}, max ${Math.round(l.maxMs / 1000)} s)`).join(" ; ") || "—"}`);
