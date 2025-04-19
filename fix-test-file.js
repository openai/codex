const fs = require('fs');
const path = require('path');

// Read the test file
const testFilePath = path.join(__dirname, 'codex-cli/src/utils/singlepass/__tests__/gitignore.test.ts');
let content = fs.readFileSync(testFilePath, 'utf8');

// Add @ts-ignore comments before all the problematic casts
content = content.replace(/\(\s*fs\.existsSync as MockedFunction<typeof fs\.existsSync>\s*\)/g, 
                         '(\n        // @ts-ignore - Type issues with mocks\n        fs.existsSync as MockedFunction<typeof fs.existsSync>\n      )');

content = content.replace(/\(\s*fs\.readFileSync as MockedFunction<typeof fs\.readFileSync>\s*\)/g, 
                         '(\n        // @ts-ignore - Type issues with mocks\n        fs.readFileSync as MockedFunction<typeof fs.readFileSync>\n      )');

// Fix the single-line casts
content = content.replace(/\(fs\.existsSync as MockedFunction<typeof fs\.existsSync>\)\.mockReturnValue/g, 
                         '(/* @ts-ignore - Type issues with mocks */ fs.existsSync as MockedFunction<typeof fs.existsSync>).mockReturnValue');

// Write the updated content back to the file
fs.writeFileSync(testFilePath, content);

console.log('Added @ts-ignore comments to the test file.');
