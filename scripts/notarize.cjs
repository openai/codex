#!/usr/bin/env node
/**
 * notarize.js
 * Usage:
 *   NODE_URL="https://sepolia.infura.io/v3/..." PRIVATE_KEY="0x..." NOTARY_CONTRACT="0x..." node scripts/notarize.cjs --hash 0x<sha256> [--cid <ipfs_cid>]
 */

const { ethers } = require("ethers");
const yargs = require("yargs/yargs");
const { hideBin } = require("yargs/helpers");
require("dotenv").config();

const argv = yargs(hideBin(process.argv))
  .option("hash", { type: "string", demandOption: true, describe: "0x-prefixed SHA256 bundle hash" })
  .option("cid", { type: "string", demandOption: false, describe: "Optional IPFS CID for the bundle" })
  .strict()
  .argv;

async function main() {
  const NODE_URL = process.env.NODE_URL;
  const PRIVATE_KEY = process.env.PRIVATE_KEY;
  const NOTARY_CONTRACT = process.env.NOTARY_CONTRACT;

  if (!NODE_URL || !PRIVATE_KEY || !NOTARY_CONTRACT) {
    console.error("Please export NODE_URL, PRIVATE_KEY, and NOTARY_CONTRACT environment variables.");
    process.exit(1);
  }

  const ABI = [
    "event Notarized(bytes32 indexed docHash, address indexed who, uint256 ts, string cid)",
    "function notarize(bytes32 docHash, string memory cid) public"
  ];

  const provider = new ethers.providers.JsonRpcProvider(NODE_URL);
  const wallet = new ethers.Wallet(PRIVATE_KEY, provider);
  const contract = new ethers.Contract(NOTARY_CONTRACT, ABI, wallet);

  const docHash = argv.hash;
  const cid = argv.cid || "";

  if (!docHash.startsWith("0x") || docHash.length !== 66) {
    console.error("Hash must be a 0x-prefixed 64-hex-character SHA256 value.");
    process.exit(1);
  }

  console.log("Submitting notarization for hash:", docHash, "CID:", cid);
  const tx = await contract.notarize(docHash, cid, { gasLimit: 200000 });
  console.log("Tx hash:", tx.hash);
  const receipt = await tx.wait();
  console.log("Notarized in block:", receipt.blockNumber);
  console.log("Event logs:", receipt.logs);
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
