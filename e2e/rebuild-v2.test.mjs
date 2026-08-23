// Tests de la décision de rebuild (PLAN-KAIZEN-CLAUDE vague 2, E1).
// La règle : le bump de mtime de main.rs (qui force la ré-expansion de
// `generate_context!`, donc recompile + link) ne se paie que si le dist
// ou tauri.conf.json ont RÉELLEMENT changé depuis le dernier build.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { empreinteDist } from './rebuild-v2.mjs';

function distDeTest(fichiers) {
  const dir = mkdtempSync(path.join(tmpdir(), 'wind-dist-'));
  for (const [rel, contenu] of Object.entries(fichiers)) {
    const abs = path.join(dir, rel);
    mkdirSync(path.dirname(abs), { recursive: true });
    writeFileSync(abs, contenu);
  }
  return dir;
}

test('la même arborescence donne la même empreinte (déterminisme)', () => {
  const a = distDeTest({ 'index.html': '<html>', 'assets/app.js': 'x=1' });
  const b = distDeTest({ 'index.html': '<html>', 'assets/app.js': 'x=1' });
  try {
    assert.equal(empreinteDist(a, 'conf'), empreinteDist(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('un contenu changé change l’empreinte', () => {
  const a = distDeTest({ 'index.html': '<html>' });
  const b = distDeTest({ 'index.html': '<html>!' });
  try {
    assert.notEqual(empreinteDist(a, 'conf'), empreinteDist(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('un nom de fichier changé change l’empreinte (assets hashés de Vite)', () => {
  const a = distDeTest({ 'assets/app-abc.js': 'x=1' });
  const b = distDeTest({ 'assets/app-def.js': 'x=1' });
  try {
    assert.notEqual(empreinteDist(a, 'conf'), empreinteDist(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('la conf Tauri fait partie de l’empreinte (taille de fenêtre du banc)', () => {
  const a = distDeTest({ 'index.html': '<html>' });
  try {
    assert.notEqual(empreinteDist(a, 'conf-1'), empreinteDist(a, 'conf-2'));
  } finally {
    rmSync(a, { recursive: true, force: true });
  }
});
