#!/usr/bin/env node

const path = require('path');
const spawn = require("child_process").spawn;
const executable = process.platform === 'win32'
  ? 'tree-sitter.exe'
  : 'tree-sitter';
spawn(
  path.join(__dirname, executable),
  process.argv.slice(2),
  {stdio: 'inherit'}
).on('close', process.exit)
