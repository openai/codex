function initializeLocalTheme() {
  const themeToggle = document.getElementById('theme-toggle');
  if (!themeToggle) return;

  // Load saved theme or use system preference
  const savedTheme = localStorage.getItem('theme');
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const initialTheme = savedTheme || (prefersDark ? 'dark' : 'light');

  // Set initial theme
  document.documentElement.setAttribute('data-theme', initialTheme);

  themeToggle.addEventListener('click', () => {
    const currentTheme = document.documentElement.getAttribute('data-theme');
    const newTheme = currentTheme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', newTheme);
    localStorage.setItem('theme', newTheme);
  });
}

function initializeCustomSelect({ initialValue = null, addListeners = false }) {
  const button = document.getElementById('language-button');
  const select = document.getElementById('language-select');
  if (!button || !select) return;

  const dropdown = button.nextElementSibling;
  const selectedValue = button.querySelector('.selected-value');

  if (initialValue) {
    select.value = initialValue;
  }
  if (select.selectedIndex >= 0 && select.options[select.selectedIndex]) {
    selectedValue.textContent = select.options[select.selectedIndex].text;
  } else {
    selectedValue.textContent = 'JavaScript';
  }

  if (addListeners) {
    button.addEventListener('click', (e) => {
      e.preventDefault(); // Prevent form submission
      dropdown.classList.toggle('show');
    });

    document.addEventListener('click', (e) => {
      if (!button.contains(e.target)) {
        dropdown.classList.remove('show');
      }
    });

    dropdown.querySelectorAll('.option').forEach(option => {
      option.addEventListener('click', () => {
        selectedValue.textContent = option.textContent;
        select.value = option.dataset.value;
        dropdown.classList.remove('show');

        const event = new Event('change');
        select.dispatchEvent(event);
      });
    });
  }
}

window.initializePlayground = async (opts) => {
  const { Parser, Language } = window.TreeSitter;

  const { local } = opts;
  if (local) {
    initializeLocalTheme();
  }
  initializeCustomSelect({ addListeners: true });

  let tree;

  const CAPTURE_REGEX = /@\s*([\w\._-]+)/g;
  const LIGHT_COLORS = [
    "#0550ae", // blue
    "#ab5000", // rust brown
    "#116329", // forest green
    "#844708", // warm brown
    "#6639ba", // purple
    "#7d4e00", // orange brown
    "#0969da", // bright blue
    "#1a7f37", // green
    "#cf222e", // red
    "#8250df", // violet
    "#6e7781", // gray
    "#953800", // dark orange
    "#1b7c83"  // teal
  ];

  const DARK_COLORS = [
    "#79c0ff", // light blue
    "#ffa657", // orange
    "#7ee787", // light green
    "#ff7b72", // salmon
    "#d2a8ff", // light purple
    "#ffa198", // pink
    "#a5d6ff", // pale blue
    "#56d364", // bright green
    "#ff9492", // light red
    "#e0b8ff", // pale purple
    "#9ca3af", // gray
    "#ffb757", // yellow orange
    "#80cbc4"  // light teal
  ];

  const codeInput = document.getElementById("code-input");
  const languageSelect = document.getElementById("language-select");
  const loggingCheckbox = document.getElementById("logging-checkbox");
  const anonymousNodes = document.getElementById('anonymous-nodes-checkbox');
  const outputContainer = document.getElementById("output-container");
  const outputContainerScroll = document.getElementById(
    "output-container-scroll",
  );
  const playgroundContainer = document.getElementById("playground-container");
  const queryCheckbox = document.getElementById("query-checkbox");
  const queryContainer = document.getElementById("query-container");
  const queryInput = document.getElementById("query-input");
  const accessibilityCheckbox = document.getElementById("accessibility-checkbox");
  const updateTimeSpan = document.getElementById("update-time");
  const languagesByName = {};

  loadState();

  await Parser.init();

  const parser = new Parser();

  console.log(parser, codeInput, queryInput);

  const codeEditor = CodeMirror.fromTextArea(codeInput, {
    lineNumbers: true,
    showCursorWhenSelecting: true
  });

  codeEditor.on('keydown', (_, event) => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.stopPropagation(); // Prevent mdBook from going back/forward
    }
  });

  const queryEditor = CodeMirror.fromTextArea(queryInput, {
    lineNumbers: true,
    showCursorWhenSelecting: true,
  });

  queryEditor.on('keydown', (_, event) => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.stopPropagation(); // Prevent mdBook from going back/forward
    }
  });

  const cluster = new Clusterize({
    rows: [],
    noDataText: null,
    contentElem: outputContainer,
    scrollElem: outputContainerScroll,
  });
  const renderTreeOnCodeChange = debounce(renderTree, 50);
  const saveStateOnChange = debounce(saveState, 2000);
  const runTreeQueryOnChange = debounce(runTreeQuery, 50);

  let languageName = languageSelect.value;
  let treeRows = null;
  let treeRowHighlightedIndex = -1;
  let parseCount = 0;
  let isRendering = 0;
  let query;

  codeEditor.on("changes", handleCodeChange);
  codeEditor.on("viewportChange", runTreeQueryOnChange);
  codeEditor.on("cursorActivity", debounce(handleCursorMovement, 150));
  queryEditor.on("changes", debounce(handleQueryChange, 150));

  loggingCheckbox.addEventListener("change", handleLoggingChange);
  anonymousNodes.addEventListener('change', renderTree);
  queryCheckbox.addEventListener("change", handleQueryEnableChange);
  accessibilityCheckbox.addEventListener("change", handleQueryChange);
  languageSelect.addEventListener("change", handleLanguageChange);
  outputContainer.addEventListener("click", handleTreeClick);

  handleQueryEnableChange();
  await handleLanguageChange();

  playgroundContainer.style.visibility = "visible";

  async function handleLanguageChange() {
    const newLanguageName = languageSelect.value;
    if (!languagesByName[newLanguageName]) {
      const url = `${LANGUAGE_BASE_URL}/tree-sitter-${newLanguageName}.wasm`;
      languageSelect.disabled = true;
      try {
        languagesByName[newLanguageName] = await Language.load(url);
      } catch (e) {
        console.error(e);
        languageSelect.value = languageName;
        return;
      } finally {
        languageSelect.disabled = false;
      }
    }

    tree = null;
    languageName = newLanguageName;
    parser.setLanguage(languagesByName[newLanguageName]);
    handleCodeChange();
    handleQueryChange();
  }

  async function handleCodeChange(editor, changes) {
    const newText = codeEditor.getValue() + "\n";
    const edits = tree && changes && changes.map(treeEditForEditorChange);

    const start = performance.now();
    if (edits) {
      for (const edit of edits) {
        tree.edit(edit);
      }
    }
    const newTree = parser.parse(newText, tree);
    const duration = (performance.now() - start).toFixed(1);

    updateTimeSpan.innerText = `${duration} ms`;
    if (tree) tree.delete();
    tree = newTree;
    parseCount++;
    renderTreeOnCodeChange();
    runTreeQueryOnChange();
    saveStateOnChange();
  }

  async function renderTree() {
    isRendering++;
    const cursor = tree.walk();

    let currentRenderCount = parseCount;
    let row = "";
    let rows = [];
    let finishedRow = false;
    let visitedChildren = false;
    let indentLevel = 0;

    for (let i = 0; ; i++) {
      if (i > 0 && i % 10000 === 0) {
        await new Promise((r) => setTimeout(r, 0));
        if (parseCount !== currentRenderCount) {
          cursor.delete();
          isRendering--;
          return;
        }
      }

      let displayName;
      if (cursor.nodeIsMissing) {
        const nodeTypeText = cursor.nodeIsNamed ? cursor.nodeType : `"${cursor.nodeType}"`;
        displayName = `MISSING ${nodeTypeText}`;
      } else if (cursor.nodeIsNamed) {
        displayName = cursor.nodeType;
      } else if (anonymousNodes.checked) {
        displayName = cursor.nodeType
      }

      if (visitedChildren) {
        if (displayName) {
          finishedRow = true;
        }

        if (cursor.gotoNextSibling()) {
          visitedChildren = false;
        } else if (cursor.gotoParent()) {
          visitedChildren = true;
          indentLevel--;
        } else {
          break;
        }
      } else {
        if (displayName) {
          if (finishedRow) {
            row += "</div>";
            rows.push(row);
            finishedRow = false;
          }
          const start = cursor.startPosition;
          const end = cursor.endPosition;
          const id = cursor.nodeId;
          let fieldName = cursor.currentFieldName;
          if (fieldName) {
            fieldName += ": ";
          } else {
            fieldName = "";
          }

          const nodeClass =
            displayName === 'ERROR' || displayName.startsWith('MISSING')
              ? 'node-link error'
              : cursor.nodeIsNamed
                ? 'node-link named'
                : 'node-link anonymous';

          row = `<div class="tree-row">${"  ".repeat(indentLevel)}${fieldName}` +
            `<a class='${nodeClass}' href="#" data-id=${id} ` +
            `data-range="${start.row},${start.column},${end.row},${end.column}">` +
            `${displayName}</a> <span class="position-info">` +
            `[${start.row}, ${start.column}] - [${end.row}, ${end.column}]</span>`;
          finishedRow = true;
        }

        if (cursor.gotoFirstChild()) {
          visitedChildren = false;
          indentLevel++;
        } else {
          visitedChildren = true;
        }
      }
    }
    if (finishedRow) {
      row += "</div>";
      rows.push(row);
    }

    cursor.delete();
    cluster.update(rows);
    treeRows = rows;
    isRendering--;
    handleCursorMovement();
  }

  function getCaptureCSS(name) {
    if (accessibilityCheckbox.checked) {
      return `color: white; background-color: ${colorForCaptureName(name)}`;
    } else {
      return `color: ${colorForCaptureName(name)}`;
    }
  }

  function runTreeQuery(_, startRow, endRow) {
    if (endRow == null) {
      const viewport = codeEditor.getViewport();
      startRow = viewport.from;
      endRow = viewport.to;
    }

    codeEditor.operation(() => {
      const marks = codeEditor.getAllMarks();
      marks.forEach((m) => m.clear());

      if (tree && query) {
        const captures = query.captures(
          tree.rootNode,
          { row: startRow, column: 0 },
          { row: endRow, column: 0 },
        );
        let lastNodeId;
        for (const { name, node } of captures) {
          if (node.id === lastNodeId) continue;
          lastNodeId = node.id;
          const { startPosition, endPosition } = node;
          codeEditor.markText(
            { line: startPosition.row, ch: startPosition.column },
            { line: endPosition.row, ch: endPosition.column },
            {
              inclusiveLeft: true,
              inclusiveRight: true,
              css: getCaptureCSS(name),
            },
          );
        }
      }
    });
  }

  // When we change from a dark theme to a light theme (and vice versa), the colors of the
  // captures need to be updated.
  const observer = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      if (mutation.attributeName === 'class') {
        handleQueryChange();
      }
    });
  });

  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class']
  });

  function handleQueryChange() {
    if (query) {
      query.delete();
      query.deleted = true;
      query = null;
    }

    queryEditor.operation(() => {
      queryEditor.getAllMarks().forEach((m) => m.clear());
      if (!queryCheckbox.checked) return;

      const queryText = queryEditor.getValue();

      try {
        query = parser.language.query(queryText);
        let match;

        let row = 0;
        queryEditor.eachLine((line) => {
          while ((match = CAPTURE_REGEX.exec(line.text))) {
            queryEditor.markText(
              { line: row, ch: match.index },
              { line: row, ch: match.index + match[0].length },
              {
                inclusiveLeft: true,
                inclusiveRight: true,
                css: `color: ${colorForCaptureName(match[1])}`,
              },
            );
          }
          row++;
        });
      } catch (error) {
        const startPosition = queryEditor.posFromIndex(error.index);
        const endPosition = {
          line: startPosition.line,
          ch: startPosition.ch + (error.length || Infinity),
        };

        if (error.index === queryText.length) {
          if (startPosition.ch > 0) {
            startPosition.ch--;
          } else if (startPosition.row > 0) {
            startPosition.row--;
            startPosition.column = Infinity;
          }
        }

        queryEditor.markText(startPosition, endPosition, {
          className: "query-error",
          inclusiveLeft: true,
          inclusiveRight: true,
          attributes: { title: error.message },
        });
      }
    });

    runTreeQuery();
    saveQueryState();
  }

  function handleCursorMovement() {
    if (isRendering) return;

    const selection = codeEditor.getDoc().listSelections()[0];
    let start = { row: selection.anchor.line, column: selection.anchor.ch };
    let end = { row: selection.head.line, column: selection.head.ch };
    if (
      start.row > end.row ||
      (start.row === end.row && start.column > end.column)
    ) {
      let swap = end;
      end = start;
      start = swap;
    }
    const node = tree.rootNode.namedDescendantForPosition(start, end);
    if (treeRows) {
      if (treeRowHighlightedIndex !== -1) {
        const row = treeRows[treeRowHighlightedIndex];
        if (row)
          treeRows[treeRowHighlightedIndex] = row.replace(
            "highlighted",
            "plain",
          );
      }
      treeRowHighlightedIndex = treeRows.findIndex((row) =>
        row.includes(`data-id=${node.id}`),
      );
      if (treeRowHighlightedIndex !== -1) {
        const row = treeRows[treeRowHighlightedIndex];
        if (row)
          treeRows[treeRowHighlightedIndex] = row.replace(
            "plain",
            "highlighted",
          );
      }
      cluster.update(treeRows);
      const lineHeight = cluster.options.item_height;
      const scrollTop = outputContainerScroll.scrollTop;
      const containerHeight = outputContainerScroll.clientHeight;
      const offset = treeRowHighlightedIndex * lineHeight;
      if (scrollTop > offset - 20) {
        $(outputContainerScroll).animate({ scrollTop: offset - 20 }, 150);
      } else if (scrollTop < offset + lineHeight + 40 - containerHeight) {
        $(outputContainerScroll).animate(
          { scrollTop: offset - containerHeight + 40 },
          150,
        );
      }
    }
  }

  function handleTreeClick(event) {
    if (event.target.tagName === "A") {
      event.preventDefault();
      const [startRow, startColumn, endRow, endColumn] =
        event.target.dataset.range.split(",").map((n) => parseInt(n));
      codeEditor.focus();
      codeEditor.setSelection(
        { line: startRow, ch: startColumn },
        { line: endRow, ch: endColumn },
      );
    }
  }

  function handleLoggingChange() {
    if (loggingCheckbox.checked) {
      parser.setLogger((message, lexing) => {
        if (lexing) {
          console.log("  ", message);
        } else {
          console.log(message);
        }
      });
    } else {
      parser.setLogger(null);
    }
  }

  function handleQueryEnableChange() {
    if (queryCheckbox.checked) {
      queryContainer.style.visibility = "";
      queryContainer.style.position = "";
    } else {
      queryContainer.style.visibility = "hidden";
      queryContainer.style.position = "absolute";
    }
    handleQueryChange();
  }

  function treeEditForEditorChange(change) {
    const oldLineCount = change.removed.length;
    const newLineCount = change.text.length;
    const lastLineLength = change.text[newLineCount - 1].length;

    const startPosition = { row: change.from.line, column: change.from.ch };
    const oldEndPosition = { row: change.to.line, column: change.to.ch };
    const newEndPosition = {
      row: startPosition.row + newLineCount - 1,
      column:
        newLineCount === 1
          ? startPosition.column + lastLineLength
          : lastLineLength,
    };

    const startIndex = codeEditor.indexFromPos(change.from);
    let newEndIndex = startIndex + newLineCount - 1;
    let oldEndIndex = startIndex + oldLineCount - 1;
    for (let i = 0; i < newLineCount; i++) newEndIndex += change.text[i].length;
    for (let i = 0; i < oldLineCount; i++)
      oldEndIndex += change.removed[i].length;

    return {
      startIndex,
      oldEndIndex,
      newEndIndex,
      startPosition,
      oldEndPosition,
      newEndPosition,
    };
  }

  function colorForCaptureName(capture) {
    const id = query.captureNames.indexOf(capture);
    const isDark = document.querySelector('html').classList.contains('ayu') ||
      document.querySelector('html').classList.contains('coal') ||
      document.querySelector('html').classList.contains('navy');

    const colors = isDark ? DARK_COLORS : LIGHT_COLORS;
    return colors[id % colors.length];
  }

  function loadState() {
    const language = localStorage.getItem("language");
    const sourceCode = localStorage.getItem("sourceCode");
    const anonNodes = localStorage.getItem("anonymousNodes");
    const query = localStorage.getItem("query");
    const queryEnabled = localStorage.getItem("queryEnabled");
    if (language != null && sourceCode != null && query != null) {
      queryInput.value = query;
      codeInput.value = sourceCode;
      languageSelect.value = language;
      initializeCustomSelect({ initialValue: language });
      anonymousNodes.checked = anonNodes === "true";
      queryCheckbox.checked = queryEnabled === "true";
    }
  }

  function saveState() {
    localStorage.setItem("language", languageSelect.value);
    localStorage.setItem("sourceCode", codeEditor.getValue());
    localStorage.setItem("anonymousNodes", anonymousNodes.checked);
    saveQueryState();
  }

  function saveQueryState() {
    localStorage.setItem("queryEnabled", queryCheckbox.checked);
    localStorage.setItem("query", queryEditor.getValue());
  }

  function debounce(func, wait, immediate) {
    var timeout;
    return function () {
      var context = this,
        args = arguments;
      var later = function () {
        timeout = null;
        if (!immediate) func.apply(context, args);
      };
      var callNow = immediate && !timeout;
      clearTimeout(timeout);
      timeout = setTimeout(later, wait);
      if (callNow) func.apply(context, args);
    };
  }
};
