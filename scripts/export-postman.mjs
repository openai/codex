import fs from "node:fs";
import path from "node:path";
import Converter from "openapi-to-postmanv2";

const inputPath = path.resolve("./smallwallets-openapi.json");
const outDir = path.resolve("./postman");
const outPath = path.join(outDir, "SmallWallets.postman_collection.json");

if (!fs.existsSync(inputPath)) {
  console.error("Missing OpenAPI JSON:", inputPath);
  process.exit(1);
}

fs.mkdirSync(outDir, { recursive: true });

const openapi = JSON.parse(fs.readFileSync(inputPath, "utf8"));

Converter.convert(
  { type: "json", data: openapi },
  { folderStrategy: "Tags", includeAuthInfoInExample: true },
  (err, result) => {
    if (err) {
      console.error("Conversion error:", err);
      process.exit(1);
    }
    if (!result.result) {
      console.error("Conversion failed:", result.reason);
      process.exit(1);
    }
    fs.writeFileSync(outPath, JSON.stringify(result.output[0].data, null, 2));
    console.log("Wrote:", outPath);
  }
);
