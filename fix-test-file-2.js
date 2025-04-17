const fs = require('fs');
const path = require('path');

// Read the test file
const testFilePath = path.join(__dirname, 'src/utils/singlepass/__tests__/gitignore.test.ts');
let content = fs.readFileSync(testFilePath, 'utf8');

// Add @ts-ignore comments before all mockImplementation calls
content = content.replace(/\)\s*\.mockImplementation\(\(p: string\) =>/g, 
                         ').mockImplementation((\n        // @ts-ignore - Type issues with mock parameters\n        p: string) =>');

// Write the updated content back to the file
fs.writeFileSync(testFilePath, content);

console.log('Added @ts-ignore comments to the mockImplementation calls in the test file.');
