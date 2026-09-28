#!/usr/bin/env node
// Validates every pack in marketplace/packs and writes marketplace/index.json,
// the catalogue the app downloads. With --check it only verifies that
// index.json is valid and up to date (CI runs this on every pull request).
// The app re-validates everything it downloads; these rules mirror
// src-tauri/src/packs.rs and src-tauri/src/marketplace.rs.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../marketplace/", import.meta.url));
const PACKS = join(ROOT, "packs");
const INDEX = join(ROOT, "index.json");

const MAX_FILES = 64;
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MAX_PACK_BYTES = 16 * 1024 * 1024;
const LICENSES = new Set(["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "MIT", "Apache-2.0"]);
const TYPES = new Set(["linear", "tactile", "clicky", "other"]);
const ID = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const AUDIO = /^([A-Za-z0-9_-]+\/)?[A-Za-z0-9_-]+\.(wav|mp3|ogg|flac)$/i;
const VERSION = /^\d+\.\d+\.\d+$/;
const SETS = ["default", "space", "enter", "backspace"];

const text = (value, min, max) =>
  typeof value === "string" && [...value].length >= min && [...value].length <= max && !/[\u0000-\u001f\u007f]/.test(value);

function walk(dir, prefix = "") {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name), `${prefix}${entry.name}/`) : [`${prefix}${entry.name}`],
  );
}

function referenced(set, where, errors) {
  if (set === undefined) return [];
  if (typeof set !== "object" || set === null || Array.isArray(set)) {
    errors.push(`${where} must be an object`);
    return [];
  }
  const paths = [];
  for (const key of Object.keys(set)) {
    if (key === "rows") {
      if (!Array.isArray(set.rows) || set.rows.length > 5 || !set.rows.every(Array.isArray)) {
        errors.push(`${where}.rows must be at most 5 lists of files`);
        continue;
      }
      paths.push(...set.rows.flat());
    } else if (SETS.includes(key)) {
      if (!Array.isArray(set[key])) errors.push(`${where}.${key} must be a list of files`);
      else paths.push(...set[key]);
    } else {
      errors.push(`${where}.${key} is not a known sound class (${[...SETS, "rows"].join(", ")})`);
    }
  }
  return paths;
}

function validate(id) {
  const errors = [];
  const dir = join(PACKS, id);
  if (!ID.test(id) || id.length > 48) errors.push("folder name must be a lowercase id like my-pack (max 48 chars)");
  const manifestPath = join(dir, "pack.json");
  if (!existsSync(manifestPath)) return { errors: ["pack.json is missing"] };

  let m;
  try {
    m = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (e) {
    return { errors: [`pack.json is not valid JSON: ${e.message}`] };
  }
  if (m.schema !== 1) errors.push("schema must be 1");
  if (m.id !== id) errors.push(`id must match the folder name "${id}"`);
  if (!text(m.name, 1, 64)) errors.push("name is required (1-64 characters)");
  if (!VERSION.test(m.version ?? "")) errors.push('version is required, like "1.0.0"');
  if (!text(m.author, 1, 200)) errors.push("author is required");
  if (!LICENSES.has(m.license)) errors.push(`license must be one of ${[...LICENSES].join(", ")}`);
  if (!TYPES.has(m.type)) errors.push(`type must be one of ${[...TYPES].join(", ")}`);
  if (m.description !== undefined && !text(m.description, 0, 300)) errors.push("description is at most 300 characters");
  if (m.source !== undefined && !text(m.source, 0, 300)) errors.push("source is at most 300 characters");

  const press = referenced(m.press, "press", errors);
  const release = referenced(m.release, "release", errors);
  if (!(m.press?.default?.length || m.press?.rows?.some((r) => r.length))) errors.push("press needs default or rows sounds");
  const sounds = [...new Set([...press, ...release])];
  for (const path of sounds) {
    if (typeof path !== "string" || !AUDIO.test(path)) errors.push(`invalid sound path ${JSON.stringify(path)}`);
    else if (!existsSync(join(dir, path))) errors.push(`${path} is referenced but missing`);
  }

  const onDisk = walk(dir).sort();
  for (const file of onDisk) {
    if (file !== "pack.json" && !sounds.includes(file)) errors.push(`${file} is not referenced by pack.json (remove it)`);
  }
  if (onDisk.length > MAX_FILES + 1) errors.push(`at most ${MAX_FILES} sound files`);

  let total = 0;
  const files = onDisk
    .filter((f) => f === "pack.json" || sounds.includes(f))
    .map((path) => {
      const bytes = readFileSync(join(dir, path));
      total += bytes.length;
      if (bytes.length === 0 || bytes.length > MAX_FILE_BYTES) errors.push(`${path} must be 1 byte to 2 MB`);
      return { path, size: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") };
    });
  if (total > MAX_PACK_BYTES) errors.push("pack is larger than 16 MB");

  const entry = {
    id,
    name: m.name,
    version: m.version,
    author: m.author,
    license: m.license,
    type: m.type,
    description: m.description ?? "",
    files,
  };
  return { entry, errors };
}

const ids = existsSync(PACKS)
  ? readdirSync(PACKS).filter((name) => statSync(join(PACKS, name)).isDirectory()).sort()
  : [];
const packs = [];
let failed = false;
for (const id of ids) {
  const { entry, errors } = validate(id);
  if (errors.length) {
    failed = true;
    console.error(`\n${id}:`);
    for (const e of errors) console.error(`  - ${e}`);
  } else {
    packs.push(entry);
  }
}
if (failed) {
  console.error("\nFix the problems above. See marketplace/README.md for the pack format.");
  process.exit(1);
}

const json = `${JSON.stringify({ schema: 1, packs }, null, 2)}\n`;
if (process.argv.includes("--check")) {
  const current = existsSync(INDEX) ? readFileSync(INDEX, "utf8").replace(/\r\n/g, "\n") : "";
  if (current !== json) {
    console.error("marketplace/index.json is out of date. Run `npm run marketplace` and commit the result.");
    process.exit(1);
  }
  console.log(`marketplace index OK (${packs.length} packs)`);
} else {
  writeFileSync(INDEX, json);
  console.log(`wrote marketplace/index.json (${packs.length} packs)`);
}
