#!/usr/bin/env node
import fs from "fs";
import path from "path";
import yaml from "js-yaml";
import { createRequire } from "module";

const require = createRequire(import.meta.url);
const Ajv = require("ajv");

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const schemaPath = path.join(repoRoot, "docs", "agent-spec.schema.json");
const agentsDir = path.join(repoRoot, "agents");

const schema = JSON.parse(fs.readFileSync(schemaPath, "utf8"));
const ajv = new Ajv();
// Support the draft 2020-12 meta-schema offline
try {
  const meta202012 = require('ajv/dist/refs/json-schema-draft-2020-12.json');
  ajv.addMetaSchema(meta202012);
} catch (e) {
  console.warn('Warning: could not load draft2020-12 meta-schema, external $ref resolution may fail');
}
const validate = ajv.compile(schema);

let failed = false;
if (!fs.existsSync(agentsDir)) process.exit(0);

for (const file of fs.readdirSync(agentsDir)) {
  if (!file.endsWith(".yaml")) continue;
  const full = path.join(agentsDir, file);
  const doc = yaml.load(fs.readFileSync(full, "utf8"));
  const ok = validate(doc);
  if (!ok) {
    console.error(`❌ ${file} failed validation:`);
    console.error(validate.errors);
    failed = true;
  }
}

if (failed) process.exit(1);
console.log("✓ agent specs valid");