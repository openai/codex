<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/codemirror.min.css">
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/clusterize.js/0.19.0/clusterize.min.css">

<h1>Syntax Tree Playground</h1>

<div id="playground-container" class="ts-playground" style="visibility: hidden;">

<h2>Code</h2>

<div class="custom-select">
  <button id="language-button" class="select-button">
    <span class="selected-value">JavaScript</span>
    <svg class="arrow" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="6 9 12 15 18 9"></polyline>
    </svg>
  </button>
  <div class="select-dropdown">
    <div class="option" data-value="bash">Bash</div>
    <div class="option" data-value="c">C</div>
    <div class="option" data-value="cpp">C++</div>
    <div class="option" data-value="c_sharp">C#</div>
    <div class="option" data-value="go">Go</div>
    <div class="option" data-value="html">HTML</div>
    <div class="option" data-value="java">Java</div>
    <div class="option" data-value="javascript">JavaScript</div>
    <div class="option" data-value="php">PHP</div>
    <div class="option" data-value="python">Python</div>
    <div class="option" data-value="ruby">Ruby</div>
    <div class="option" data-value="rust">Rust</div>
    <div class="option" data-value="toml">TOML</div>
    <div class="option" data-value="typescript">TypeScript</div>
    <div class="option" data-value="yaml">YAML</div>
  </div>
  <select id="language-select" style="display: none;">
    <option value="bash">Bash</option>
    <option value="c">C</option>
    <option value="cpp">C++</option>
    <option value="c_sharp">C#</option>
    <option value="go">Go</option>
    <option value="html">HTML</option>
    <option value="java">Java</option>
    <option value="javascript" selected="selected">JavaScript</option>
    <option value="php">PHP</option>
    <option value="python">Python</option>
    <option value="ruby">Ruby</option>
    <option value="rust">Rust</option>
    <option value="toml">TOML</option>
    <option value="typescript">TypeScript</option>
    <option value="yaml">YAML</option>
  </select>
</div>

<input id="logging-checkbox" type="checkbox"></input>
<label for="logging-checkbox">Log</label>

<input id="anonymous-nodes-checkbox" type="checkbox"></input>
<label for="anonymous-nodes-checkbox">Show anonymous nodes</label>

<input id="query-checkbox" type="checkbox"></input>
<label for="query-checkbox">Query</label>

<input id="accessibility-checkbox" type="checkbox"></input>
<label for="accessibility-checkbox">Accessibility</label>

<textarea id="code-input">
</textarea>

<div id="query-container" style="visibility: hidden; position: absolute;">
<h2>Query</h2>
<textarea id="query-input"></textarea>
</div>

<h2>Tree</h2>
<span id="update-time"></span>
<div id="output-container-scroll">
<pre id="output-container" class="highlight"></pre>
</div>

<h2 id="about">About </h2>
<p>You can try out tree-sitter with a few pre-selected grammars on this page.
You can also run playground locally (with your own grammar) using the
<a href="/cli/playground.html">CLI</a>'s <code>tree-sitter playground</code> subcommand.
</p>

```admonish info
Logging (if enabled) can be viewed in the browser's console.
```

<p>The syntax tree should update as you type in the code. As you move around the
code, the current node should be highlighted in the tree; you can also click any
node in the tree to select the corresponding part of the code.</p>
<p>You can enter one or more <a href="/tree-sitter/using-parsers/queries/index.html">patterns</a>
into the Query panel. If the query is valid, its captures will be
highlighted both in the Code and in the Query panels. Otherwise
the problematic parts of the query will be underlined, and detailed
diagnostics will be available on hover. Note that to see any results
you must use at least one capture, like <code>(node_name) @capture-name</code></p>

</div>

<script src="https://cdnjs.cloudflare.com/ajax/libs/codemirror/6.65.7/codemirror.min.js"></script>

<script>LANGUAGE_BASE_URL = "https://tree-sitter.github.io";</script>
<script type="module">
import * as TreeSitter from 'https://tree-sitter.github.io/tree-sitter.js';
window.TreeSitter = TreeSitter;
setTimeout(() => window.initializePlayground({local: false}), 1);
</script>

<script src="https://cdnjs.cloudflare.com/ajax/libs/clusterize.js/0.19.0/clusterize.min.js"></script>
