const fs = require('fs');
const path = require('path');

// Read the test file
const testFilePath = path.join(__dirname, 'codex-cli/src/utils/singlepass/__tests__/gitignore.test.ts');
let content = fs.readFileSync(testFilePath, 'utf8');

// Replace all instances of (fs.existsSync as any) with (fs.existsSync as MockedFunction<typeof fs.existsSync>)
content = content.replace(/\(fs\.existsSync as any\)/g, '(fs.existsSync as MockedFunction<typeof fs.existsSync>)');

// Replace all instances of (fs.readFileSync as any) with (fs.readFileSync as MockedFunction<typeof fs.readFileSync>)
content = content.replace(/\(fs\.readFileSync as any\)/g, '(fs.readFileSync as MockedFunction<typeof fs.readFileSync>)');

// Write the updated content back to the file
fs.writeFileSync(testFilePath, content);

console.log('Updated all mock instances in the test file.');
