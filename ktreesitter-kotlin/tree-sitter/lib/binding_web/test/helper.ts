import { Parser, Language } from '../src';
import path from 'path';

// https://github.com/tree-sitter/tree-sitter/blob/master/xtask/src/fetch.rs#L15
export type LanguageName = 'bash' | 'c' | 'cpp' | 'embedded-template' | 'go' | 'html' | 'java' | 'javascript' | 'jsdoc' | 'json' | 'php' | 'python' | 'ruby' | 'rust' | 'typescript' | 'tsx';

function languageURL(name: LanguageName): string {
  const basePath = process.cwd();
  return path.join(basePath, `../../target/release/tree-sitter-${name}.wasm`);
}

export default Parser.init().then(async () => ({
  languageURL,
  C: await Language.load(languageURL('c')),
  EmbeddedTemplate: await Language.load(languageURL('embedded-template')),
  HTML: await Language.load(languageURL('html')),
  JavaScript: await Language.load(languageURL('javascript')),
  JSON: await Language.load(languageURL('json')),
  Python: await Language.load(languageURL('python')),
  Rust: await Language.load(languageURL('rust')),
}));
