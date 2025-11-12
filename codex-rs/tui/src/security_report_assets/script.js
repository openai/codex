(function () {
  // CSS.escape polyfill for older browsers
  if (!window.CSS || typeof window.CSS.escape !== 'function') {
    window.CSS = window.CSS || {};
    window.CSS.escape = function (value) {
      return String(value).replace(/[^a-zA-Z0-9_\-]/g, function (ch) {
        const hex = ch.charCodeAt(0).toString(16);
        return '\\' + hex + ' ';
      });
    };
  }
  const contentEl = document.getElementById('content');
  const navList = document.getElementById('navList');
  const tocList = document.getElementById('tocList');
  const clearTocBtn = document.getElementById('clearToc');
  const jobProgressHost = document.getElementById('jobProgressHost');
  const fileInput = document.getElementById('fileInput');
  const dropOverlay = document.getElementById('dropOverlay');
  const themeToggle = document.getElementById('themeToggle');
  const shareBtn = document.getElementById('shareBtn');
  const editToggle = document.getElementById('editToggle');
  const sitePath = document.getElementById('site-path');
  const navToggle = document.getElementById('navToggle');
  const sectionSearch = document.getElementById('sectionSearch');

  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)');
  function setTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
    try { localStorage.setItem('theme', theme); } catch {}
    updateThemeUI(theme);
  }
  function getSavedTheme() {
    try { return localStorage.getItem('theme'); } catch { return null; }
  }
  function currentTheme() {
    const saved = getSavedTheme();
    return saved || (prefersDark.matches ? 'dark' : 'light');
  }
  function initTheme() {
    setTheme(currentTheme());
  }
  function updateThemeUI(theme) {
    // Update toggle button state and icon
    if (!themeToggle) return;
    const isDark = theme === 'dark';
    themeToggle.setAttribute('aria-pressed', String(isDark));
    themeToggle.setAttribute('title', isDark ? 'Switch to light mode' : 'Switch to dark mode');
    themeToggle.setAttribute('aria-label', isDark ? 'Switch to light mode' : 'Switch to dark mode');
    try {
      const icon = document.getElementById('themeIcon');
      if (icon) {
        // Simple sun/moon paths; keep present default if any issue
        // Sun icon when light mode, moon icon when dark mode
        const sun = 'M6.76 4.84l-1.8-1.79-1.42 1.41 1.79 1.8 1.43-1.42zm10.48 0l1.8-1.79 1.42 1.41-1.79 1.8-1.43-1.42zM12 4V1h-2v3h2zm0 19v-3h-2v3h2zm7-9h3v-2h-3v2zM1 12H4v-2H1v2zm14.24 7.16l1.8 1.79 1.42-1.41-1.79-1.8-1.43 1.42zM4.84 17.24l-1.79 1.8 1.41 1.41 1.8-1.79-1.42-1.42zM12 6a6 6 0 100 12 6 6 0 000-12z';
        const moon = 'M21.64 13a1 1 0 0 0-1.11-.27 8 8 0 0 1-10.26-10.26 1 1 0 0 0-1.38-1.26 10 10 0 1 0 13 13 1 1 0 0 0-.25-1.21Z';
        icon.setAttribute('d', isDark ? sun : moon);
      }
    } catch {}
  }
  // Toggle theme and re-render theme-sensitive elements
  themeToggle && themeToggle.addEventListener('click', () => {
    const next = currentTheme() === 'dark' ? 'light' : 'dark';
    setTheme(next);
    try { enhanceMermaid(); } catch {}
  });
  // If user hasn't manually chosen a theme (no saved), follow OS changes
  try {
    prefersDark.addEventListener('change', (e) => {
      if (!getSavedTheme()) {
        setTheme(e.matches ? 'dark' : 'light');
        try { enhanceMermaid(); } catch {}
      }
    });
  } catch {}

  fileInput.addEventListener('change', async (e) => {
    const t = e.target;
    const file = t && t.files && t.files[0];
    if (file) await loadFile(file);
  });

  ;['dragenter', 'dragover'].forEach(evt => {
    window.addEventListener(evt, (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropOverlay.classList.add('show');
      dropOverlay.setAttribute('aria-hidden', 'false');
    });
  });
  ;['dragleave', 'drop'].forEach(evt => {
    window.addEventListener(evt, (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropOverlay.classList.remove('show');
      dropOverlay.setAttribute('aria-hidden', 'true');
    });
  });
  window.addEventListener('drop', async (e) => {
    const dt = e.dataTransfer;
    const file = dt && dt.files && dt.files[0];
    if (file && file.name.match(/\.(md|markdown|txt)$/i)) {
      await loadFile(file);
    }
  });

  // Sidebar collapse/expand
  function setSidebarCollapsed(collapsed) {
    const cls = 'sidebar-collapsed';
    document.body.classList.toggle(cls, !!collapsed);
    if (navToggle) {
      navToggle.setAttribute('aria-pressed', collapsed ? 'true' : 'false');
      navToggle.setAttribute('aria-label', collapsed ? 'Expand sidebar' : 'Collapse sidebar');
      navToggle.setAttribute('title', collapsed ? 'Expand sidebar' : 'Collapse sidebar');
      try {
        const icon = document.getElementById('navIcon');
        // Chevron-right when collapsed (to expand), chevron-left when expanded
        const right = 'M9 6l6 6-6 6';
        const left = 'M15 6l-6 6 6 6';
        if (icon) icon.setAttribute('d', collapsed ? right : left);
      } catch {}
    }
    try { localStorage.setItem('sidebar:collapsed', collapsed ? '1' : '0'); } catch {}
  }
  if (navToggle) {
    navToggle.addEventListener('click', () => {
      const collapsed = document.body.classList.contains('sidebar-collapsed');
      setSidebarCollapsed(!collapsed);
    });
  }

  // Section search events
  if (sectionSearch) {
    sectionSearch.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleSectionSearch(sectionSearch.value);
      }
    });
    // Render TOC with filtered results on input; also jump on exact match
    sectionSearch.addEventListener('input', () => {
      const v = (sectionSearch.value || '').trim();
      renderFilteredNavigation(v);
      const low = v.toLowerCase();
      if (!low) return;
      for (const i of headingIndex) {
        if ((i.text || '').trim().toLowerCase() === low) {
          handleSectionSearch(v);
          break;
        }
      }
    });
  }

  shareBtn.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(location.href);
      shareBtn.textContent = 'Link copied!';
      setTimeout(() => (shareBtn.textContent = 'Share'), 1200);
    } catch (e) {
      alert('Copy failed. You can manually copy the URL.');
    }
  });

  // Clear selection in sidebar (Outline)
  if (clearTocBtn) {
    clearTocBtn.addEventListener('click', () => {
      // Remove active highlight
      tocList && tocList.querySelectorAll('.active').forEach(el => el.classList.remove('active'));
      // Clear hash and scroll to top
      try {
        const url = new URL(location.href);
        url.hash = '';
        history.replaceState(null, '', url.toString());
      } catch {}
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }

  // Sidebar Job Progress integration: subscribes to current job via SSE
  let jp = null; let jpES = null;
  function initJobProgress() {
    if (!jobProgressHost) return;
    jobProgressHost.innerHTML = '';
    const box = document.createElement('div');
    box.className = 'job-progress';
    box.innerHTML = `
      <div class="jp-title">
        <span>Job progress</span>
        <button class="btn small" id="jpClearBtn" type="button">Hide</button>
      </div>
      <div class="jp-status" id="jpStatus">No active job</div>
      <div class="jp-bar"><span id="jpBar"></span></div>
      <div class="jp-logs" id="jpLogs"></div>
    `;
    jobProgressHost.appendChild(box);
    jp = {
      box,
      status: box.querySelector('#jpStatus'),
      bar: box.querySelector('#jpBar'),
      logs: box.querySelector('#jpLogs'),
      clearBtn: box.querySelector('#jpClearBtn'),
    };
    jp.clearBtn.addEventListener('click', () => {
      unsubscribeJobProgress();
      jp.status.textContent = 'No active job';
      jp.bar.style.width = '0%';
      jp.logs.textContent = '';
    });
    // Attach to existing job if present
    try {
      const current = localStorage.getItem('job:current');
      if (current) subscribeJobProgress(current);
    } catch {}
  }

  function subscribeJobProgress(id) {
    unsubscribeJobProgress();
    if (!id || !jp) return;
    jp.status.textContent = 'Running (Job ' + id + ')';
    try {
      jpES = new EventSource('/api/events/' + id);
      jpES.onmessage = (e) => {
        try { var d = JSON.parse(e.data); } catch { var d = e.data; }
        if (typeof d === 'object' && d) {
          if (d.event === 'close') {
            jp.status.textContent = 'Finished (code ' + d.code + ')';
            jp.bar.style.width = '100%';
            try { localStorage.removeItem('job:current'); } catch {}
            setTimeout(unsubscribeJobProgress, 3000);
            return;
          }
          if (d.line) {
            const line = d.line;
            if (/progress\s+(\d+)%/i.test(line)) {
              const m = line.match(/(\d+)%/); if (m) jp.bar.style.width = m[1] + '%';
            }
            const atBottom = jp.logs.scrollTop + jp.logs.clientHeight >= jp.logs.scrollHeight - 5;
            jp.logs.textContent += (d.source === 'stderr' ? '[ERR] ' : '') + line + '\n';
            if (atBottom) jp.logs.scrollTop = jp.logs.scrollHeight;
          }
        }
      };
      jpES.onerror = () => { jp.status.textContent = 'Disconnected'; };
    } catch {}
  }

  function unsubscribeJobProgress() {
    if (jpES) { try { jpES.close(); } catch {} jpES = null; }
  }

  // Listen for job start from job.html
  window.addEventListener('storage', (e) => {
    if (e.key === 'job:current' && e.newValue && jp) {
      subscribeJobProgress(e.newValue);
    }
  });

  // Edit mode toggle
  if (editToggle) {
    editToggle.addEventListener('click', () => {
      const isEditing = contentEl.isContentEditable;
      if (!isEditing) {
        contentEl.contentEditable = 'true';
        contentEl.classList.add('editing');
        editToggle.setAttribute('aria-pressed', 'true');
        editToggle.textContent = 'Done';
      } else {
        contentEl.contentEditable = 'false';
        contentEl.classList.remove('editing');
        editToggle.setAttribute('aria-pressed', 'false');
        editToggle.textContent = 'Edit';
        // Postprocess edited HTML to refresh TOC, IDs, diagrams, etc.
        postprocessEditedContent();
      }
    });
  }

  if (window.marked) {
    marked.setOptions({
      mangle: false,
      headerIds: true,
      headerPrefix: '',
      breaks: false,
      gfm: true,
      highlight: function (code, lang) {
        if (window.hljs) {
          if (lang && hljs.getLanguage(lang)) {
            return hljs.highlight(code, { language: lang }).value;
          }
          return hljs.highlightAuto(code).value;
        }
        return code;
      },
    });
  }

  async function loadFile(file) {
    const text = await file.text();
    renderMarkdown(text);
    sitePath.textContent = `/ Docs / ${file.name}`;
    try {
      sessionStorage.setItem('doc:' + file.name, text);
    } catch {}
    const url = new URL(location.href);
    url.searchParams.set('file', encodeURIComponent(file.name));
    history.replaceState(null, '', url.toString());
  }

  function slugify(str) {
    return str
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s-]/g, '')
      .replace(/\s+/g, '-')
      .replace(/-+/g, '-');
  }

  function renderMarkdown(text) {
    // Keep the raw Markdown around so we can copy exact sections (including code fences)
    window.__originalMarkdown = text;
    const usedMarked = !!window.marked;
    const rawHtml = usedMarked ? marked.parse(text) : basicMarkdown(text);
    contentEl.innerHTML = rawHtml;

    // Fix up any ordered lists that were split by intervening blocks
    // (e.g., fenced code or diagrams) and ended up as plain paragraphs.
    try { fixLooseOrderedLists(); } catch {}

    const headings = Array.from(contentEl.querySelectorAll('h1, h2, h3, h4, h5, h6'));
    const used = new Set();
    headings.forEach(h => {
      if (!h.id) {
        let id = slugify(h.textContent || '');
        let i = 2;
        while (used.has(id) || document.getElementById(id)) {
          id = `${id}-${i++}`;
        }
        h.id = id;
        used.add(id);
      }
    });

    enhanceCodeBlocks();

    // Syntax highlighting for fallback renderer
    if (!usedMarked && window.hljs) {
      try {
        contentEl.querySelectorAll('pre code').forEach(function (el) { hljs.highlightElement(el); });
      } catch {}
    }

    // Mark inline code for color styling only in safe contexts
    applyInlineCodeColor();

    // Remove empty bullets (list items without content)
    removeEmptyBullets();

    enhanceCallouts();

    // Mermaid diagrams
    enhanceMermaid();

    // Severity badges in tables
    enhanceSeverityBadges();

    // Link bug table titles to detail sections
    enhanceBugTableLinks();

    buildNavigation(headings);
    updateSectionSearch();

    if (location.hash && location.hash.length > 1) {
      const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
      if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }

    initScrollSpy();

    // Add per-bug floating actions
    enhanceBugTicketUI();
  }

  // Convert top-level paragraphs that begin with a numeric marker (e.g.,
  // "2. Title") into an ordered list, absorbing following siblings until the
  // next numbered paragraph or a heading. This helps when the Markdown parser
  // fails to keep a single <ol> across interspersed blocks.
  function fixLooseOrderedLists() {
    const container = contentEl;
    let currentOl = null;
    let currentLi = null;

    function closeList() {
      currentLi = null;
      currentOl = null;
    }

    function startList(startAt, beforeNode) {
      currentOl = document.createElement('ol');
      if (startAt && startAt !== 1) currentOl.setAttribute('start', String(startAt));
      container.insertBefore(currentOl, beforeNode);
    }

    // Iterate over a snapshot of children; nodes may move during the loop
    let i = 0;
    while (i < container.childNodes.length) {
      const node = container.childNodes[i];
      if (!node) { i += 1; continue; }

      const isHeading = node.nodeType === 1 && /^H[1-6]$/.test(node.nodeName);
      if (isHeading && currentOl) { closeList(); i += 1; continue; }

      let markerMatch = null;
      if (node.nodeType === 1 && node.nodeName === 'P') {
        const txt = (node.textContent || '').trim();
        markerMatch = txt.match(/^(\d+)\.\s+(.*)$/);
      }

      if (markerMatch) {
        const start = parseInt(markerMatch[1], 10) || 1;
        if (!currentOl) startList(start, node);
        else if (!currentLi && start && start !== 1) currentOl.setAttribute('start', String(start));

        // Begin a new list item and move paragraph contents into it
        currentLi = document.createElement('li');
        const html = (node.innerHTML || '').replace(/^\s*\d+\.\s+/, '');
        currentLi.innerHTML = html;
        currentOl.appendChild(currentLi);
        // Remove marker paragraph from the container
        container.removeChild(node);
        // Do not advance index; the next sibling slid into this index
        continue;
      }

      if (currentOl) {
        // Absorb non-heading siblings (paragraphs, code blocks, diagrams, lists)
        // into the current list item until next numbered paragraph or heading.
        if (!currentLi) { closeList(); i += 1; continue; }
        currentLi.appendChild(node);
        // Node moved from container; do not increment i
        continue;
      }

      i += 1;
    }
  }

  // no cleanup — we rely on correct Markdown

  function applyInlineCodeColor() {
    // Reset previous marks
    contentEl.querySelectorAll('code.inline-code').forEach(function (c) { c.classList.remove('inline-code'); });
    // Mark inline code in safe, readable contexts (paragraphs and blockquotes)
    const candidates = contentEl.querySelectorAll('p code, blockquote p code');
    candidates.forEach(function (codeEl) {
      if (codeEl.closest('pre')) return;
      if (codeEl.closest('h1,h2,h3,h4,h5,h6')) return;
      if (codeEl.closest('summary')) return;
      if (codeEl.closest('.callout-header')) return;
      codeEl.classList.add('inline-code');
    });
  }

  async function fetchAndRender(url, fallbackText) {
    try {
      const res = await fetch(url, { cache: 'no-cache' });
      if (!res.ok) throw new Error('HTTP ' + res.status);
      const txt = await res.text();
      renderMarkdown(txt);
      sitePath.textContent = `/ Docs / ${decodeURIComponent(url.split('/').pop() || url)}`;
      return true;
    } catch (e) {
      if (fallbackText) renderMarkdown(fallbackText);
      return false;
    }
  }

  function enhanceCodeBlocks() {
    const blocks = contentEl.querySelectorAll('pre > code');
    blocks.forEach(code => {
      const pre = code.parentElement;
      if (!pre) return;
      // Remove any existing copy buttons to avoid duplicates
      pre.querySelectorAll('.copy-btn').forEach(btn => btn.remove());
      const btn = document.createElement('button');
      btn.className = 'copy-btn';
      btn.type = 'button';
      btn.textContent = 'Copy';
      btn.addEventListener('click', async () => {
        try {
          await navigator.clipboard.writeText(code.innerText);
          btn.textContent = 'Copied';
          setTimeout(() => (btn.textContent = 'Copy'), 1200);
        } catch {}
      });
      pre.appendChild(btn);
    });
  }

  function postprocessEditedContent() {
    // Ensure heading IDs and rebuild ToC
    const headings = Array.from(contentEl.querySelectorAll('h1, h2, h3, h4, h5, h6'));
    const used = new Set();
    headings.forEach(h => {
      if (!h.id || used.has(h.id)) {
        let id = slugify(h.textContent || '');
        let i = 2;
        while (used.has(id) || document.getElementById(id)) {
          id = `${id}-${i++}`;
        }
        h.id = id;
      }
      used.add(h.id);
    });
    // Re-run enhancers
    enhanceCodeBlocks();
    applyInlineCodeColor();
    removeEmptyBullets();
    enhanceCallouts();
    enhanceMermaid();
    enhanceSeverityBadges();
    buildNavigation(headings);
    updateSectionSearch();
    initScrollSpy();
  }

  function removeEmptyBullets() {
    const items = contentEl.querySelectorAll('li');
    items.forEach(function (li) {
      // If there are no element children and no text content, remove the list item
      const hasElements = li.children && li.children.length > 0;
      const text = (li.textContent || '').trim();
      if (!hasElements && text === '') {
        const parent = li.parentElement;
        li.remove();
        // If the parent list becomes empty, remove it as well
        if (parent && parent.children.length === 0) parent.remove();
      }
    });
  }

  function enhanceSeverityBadges() {
    const tables = contentEl.querySelectorAll('table');
    tables.forEach(function (table) {
      // First: if a Severity column exists, badge that column
      const headerCells = table.querySelectorAll('thead tr th');
      let sevIndex = -1;
      headerCells.forEach(function (th, idx) {
        const t = (th.textContent || '').trim().toLowerCase();
        if (t === 'severity') sevIndex = idx;
      });
      if (sevIndex !== -1) {
        table.querySelectorAll('tbody tr').forEach(function (tr) {
          const cells = tr.querySelectorAll('td');
          if (cells.length <= sevIndex) return;
          badgeCellIfSeverity(cells[sevIndex]);
        });
      }
      // Then: badge any other cells that contain plain severity words
      table.querySelectorAll('tbody td').forEach(function (td) {
        badgeCellIfSeverity(td);
      });
    });
  }

  function badgeCellIfSeverity(td) {
    const raw = (td.textContent || '').trim().toLowerCase();
    let cls = '';
    let label = '';
    if (raw === 'high') { cls = 'sev-high'; label = 'High'; }
    else if (raw === 'medium' || raw === 'med') { cls = 'sev-med'; label = 'Medium'; }
    else if (raw === 'low') { cls = 'sev-low'; label = 'Low'; }
    else if (raw === 'ignore' || raw === 'ignored') { cls = 'sev-ign'; label = 'Ignore'; }
    if (!cls) return;
    td.innerHTML = '';
    const span = document.createElement('span');
    span.className = 'badge ' + cls;
    span.textContent = label;
    td.appendChild(span);
  }

  function enhanceBugTableLinks() {
    // Build a map from bug numeric ID -> heading id
    const bugHeadMap = new Map();
    Array.from(contentEl.querySelectorAll('h1, h2, h3, h4, h5, h6')).forEach(h => {
      const txt = (h.textContent || '').trim();
      const m = txt.match(/^\s*\[(\d+)\]/);
      if (m && h.id) bugHeadMap.set(m[1], h.id);
    });
    if (bugHeadMap.size === 0) return;

    const tables = contentEl.querySelectorAll('table');
    tables.forEach(table => {
      const headerCells = table.querySelectorAll('thead tr th');
      if (!headerCells.length) return;
      let idIdx = -1, titleIdx = -1;
      headerCells.forEach((th, i) => {
        const t = (th.textContent || '').trim().toLowerCase();
        if (t === 'bug id' || t === 'id') idIdx = i;
        // Support various column names that contain the bug title/description
        if (t === 'title' || t === 'component' || t === 'description') titleIdx = i;
      });
      if (idIdx === -1 || titleIdx === -1) return;

      table.querySelectorAll('tbody tr').forEach(tr => {
        const cells = tr.querySelectorAll('td');
        if (cells.length <= Math.max(idIdx, titleIdx)) return;
        const idCell = cells[idIdx];
        const titleCell = cells[titleIdx];
        const bugIdText = (idCell.textContent || '').trim();
        const bugNumMatch = bugIdText.match(/\d+/);
        if (!bugNumMatch) return;
        const bugNum = bugNumMatch[0];
        let targetId = bugHeadMap.get(bugNum);
        if (!targetId) {
          // Fallback: find a heading whose text (without leading [id]) matches the title text
          const norm = (s) => (s || '').toLowerCase().replace(/^\s*\[[0-9]+\]\s*/, '').replace(/[^a-z0-9]+/g, ' ').trim();
          const want = norm(titleText);
          if (want) {
            const heads = Array.from(contentEl.querySelectorAll('h1, h2, h3, h4, h5, h6'));
            for (const h of heads) {
              const ht = norm(h.textContent || '');
              if (ht && (ht === want || ht.startsWith(want) || want.startsWith(ht))) {
                targetId = h.id || '';
                if (targetId) break;
              }
            }
          }
          if (!targetId) return;
        }

        const titleText = (titleCell.textContent || '').trim();
        titleCell.innerHTML = '';
        const a = document.createElement('a');
        a.href = `#${targetId}`;
        a.textContent = titleText;
        a.addEventListener('click', (e) => {
          e.preventDefault();
          const el = document.getElementById(targetId);
          if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
          history.replaceState(null, '', `#${targetId}`);
        });
        titleCell.appendChild(a);
      });
    });
  }

  function enhanceBugTicketUI() {
    // Find bug headings like "[1] ..." typically h2/h3
    const bugHeads = Array.from(contentEl.querySelectorAll('h2, h3')).filter(h => /^\s*\[\d+\]/.test((h.textContent || '').trim()));
    bugHeads.forEach(h => {
      const full = (h.textContent || '').trim();
      const idMatch = full.match(/^\s*\[(\d+)\]\s*(.*)$/);
      const bugId = idMatch ? idMatch[1] : '';
      const bugTitle = idMatch ? idMatch[2] : full;

      // Prevent duplicates if reprocessing
      if (h.nextElementSibling && h.nextElementSibling.classList && h.nextElementSibling.classList.contains('ticket-box')) return;

      // Find assignee/author in subsequent paragraphs until next heading
      let author = '';
      let authorId = '';
      let node = h.nextElementSibling;
      while (node) {
        if (/^H[1-6]$/.test(node.tagName)) break;
        if (node.tagName === 'P') {
          const t = (node.textContent || '').trim();
          const m = t.match(/(?:^|\b)(?:Author|Assignee|Owner)\s*:\s*([^,(\n]+)(?:\s*\(([^)]+)\))?/i);
          if (m) {
            author = (m[1] || '').trim();
            authorId = (m[2] || '').trim();
            break;
          }
        }
        node = node.nextElementSibling;
      }
      if (!author) author = 'Unknown';

      // Build UI box
      const box = document.createElement('div');
      box.className = 'ticket-box';
      box.setAttribute('role', 'region');
      box.setAttribute('aria-label', 'Create ticket');

      const labelAssignee = document.createElement('label');
      labelAssignee.textContent = 'Assignee';
      const input = document.createElement('input');
      input.type = 'text';
      input.value = author;
      if (author && author !== 'Unknown') input.dataset.assigneeName = author;
      if (authorId && /^usr_[A-Za-z0-9]+$/.test(authorId)) input.dataset.assigneeId = authorId;
      input.setAttribute('aria-label', 'Assignee');
      labelAssignee.appendChild(input);

      const labelTeam = document.createElement('label');
      labelTeam.textContent = 'Team/Project';
      const select = document.createElement('select');
      const projects = [
        { label: 'PSEC',   value: 'https://linear.new?team=PSEC' },
        { label: 'SEC',    value: 'https://linear.new?team=SEC' },
        { label: 'ENG',    value: 'https://linear.new?team=ENG' },
        { label: 'SRE',    value: 'https://linear.new?team=SRE' },
        { label: 'PROD',   value: 'https://linear.new?team=PROD' },
        { label: 'DESIGN', value: 'https://linear.new?team=DESIGN' },
      ];
      projects.forEach(p => {
        const o = document.createElement('option');
        o.value = p.value; o.textContent = p.label;
        if (p.label === 'PSEC') o.selected = true;
        select.appendChild(o);
      });
      labelTeam.appendChild(select);

      const btn = document.createElement('button');
      btn.className = 'ticket-btn';
      btn.type = 'button';
      btn.textContent = 'Create ticket';

      const status = document.createElement('span');
      status.className = 'ticket-status';

      // Copy Markdown button
      const copyBtn = document.createElement('button');
      copyBtn.className = 'btn small';
      copyBtn.type = 'button';
      copyBtn.textContent = 'Copy markdown';
      copyBtn.addEventListener('click', async () => {
        try {
          const md = getBugMarkdownSection(window.__originalMarkdown || '', h, bugId);
          if (!md) throw new Error('Section not found');
          await navigator.clipboard.writeText(md);
          copyBtn.textContent = 'Copied!';
          setTimeout(() => { copyBtn.textContent = 'Copy markdown'; }, 1200);
        } catch (e) {
          // Fallback: copy rendered text content
          try {
            const txt = getRenderedSectionText(h);
            await navigator.clipboard.writeText(txt);
            copyBtn.textContent = 'Copied!';
            setTimeout(() => { copyBtn.textContent = 'Copy markdown'; }, 1200);
          } catch {}
        }
      });

      btn.addEventListener('click', () => {
        const title = (bugTitle || '').trim() || 'New issue';
        let description = getBugMarkdownSection(window.__originalMarkdown || '', h, bugId);
        if (!description) description = getRenderedSectionText(h);
        // Remove the heading/title from the ticket body
        (function stripHeadingFromBody(){
          const stripHeading = (text) => {
            if (!text) return text;
            const lines = String(text).split(/\r?\n/);
            while (lines.length && lines[0].trim() === '') lines.shift();
            if (!lines.length) return '';
            const first = lines[0].trim();
            const headTxt = (h.textContent || '').trim();
            if (/^#{1,6}\s+/.test(first)) {
              lines.shift();
            } else if (headTxt && first === headTxt) {
              lines.shift();
            } else if (bugId && first.startsWith(`[${bugId}]`)) {
              lines.shift();
            }
            while (lines.length && lines[0].trim() === '') lines.shift();
            return lines.join('\n').trim();
          };
          description = stripHeading(description);
          if (!description) description = '';
        })();
        const rawInput = (input.value || '').trim();
        const sanitized = rawInput.replace(/^`|`$/g, '').trim();
        const assigneeVal = sanitized || author || '';
        const enc = (s) => encodeURIComponent(String(s)).replace(/%20/g, '+');
        let assigneeParam = '';
        if (assigneeVal) {
          const initialName = input.dataset.assigneeName || '';
          const initialId = input.dataset.assigneeId || '';
          if (initialId && sanitized === initialName) {
            assigneeParam = `&assigneeId=${enc(initialId)}`;
          } else if (/^usr_[A-Za-z0-9]+$/.test(assigneeVal)) {
            assigneeParam = `&assigneeId=${enc(assigneeVal)}`;
          } else {
            assigneeParam = `&assignee=${enc(assigneeVal)}`;
          }
        }
        // Determine base from the Team/Project dropdown value. If it's a URL, use it;
        // otherwise, fall back to Linear with a team hint.
        let base = (select && select.value) || 'https://linear.new';
        if (!/^https?:\/\//i.test(base)) {
          base = `https://linear.new?team=${enc(base)}`;
        }
        // If team is PSEC, default project to Vulnerabilities
        const teamLabel = (select && select.options && select.options[select.selectedIndex]
          && select.options[select.selectedIndex].textContent) || '';
        const projectParam = teamLabel === 'PSEC' ? `&project=${enc('Vulnerabilities')}` : '';
        // Always include a default label to help triage
        const labelParam = `&label=${enc('appsec-agent-bugs')}`;
        const params = `title=${enc(title)}&description=${enc(description)}${assigneeParam}${labelParam}${projectParam}`;
        const joiner = base.includes('?') ? (/[?&]$/.test(base) ? '' : '&') : '?';
        const url = base + joiner + params;
        // Open in a new tab reliably using an anchor element within the click handler gesture
        try {
          const a = document.createElement('a');
          a.href = url;
          a.target = '_blank';
          a.rel = 'noopener noreferrer';
          a.style.display = 'none';
          document.body.appendChild(a);
          a.click();
          setTimeout(() => { try { a.remove(); } catch {} }, 0);
        } catch (e) {
          // Fallback to window.open (still attempts a new tab)
          const win = window.open(url, '_blank', 'noopener,noreferrer');
          if (!win) location.href = url;
        }
      });

      box.appendChild(labelAssignee);
      box.appendChild(labelTeam);
      box.appendChild(copyBtn);
      box.appendChild(btn);
      box.appendChild(status);

      // Insert after heading
      h.insertAdjacentElement('afterend', box);
    });
  }

  // Extract the full Markdown for a bug section by matching the bug heading and scanning until next heading
  function getBugMarkdownSection(md, headingEl, bugId) {
    if (!md || !headingEl) return '';
    const level = (function(tag){ try { return parseInt(tag.replace(/[^0-9]/g,'') || '0', 10); } catch { return 0; } })(headingEl.tagName || '');
    if (!level) return '';
    // Scan the markdown line by line to locate the heading that contains [bugId]
    let pos = 0;
    let start = -1;
    let end = md.length;
    let insideFence = false;
    let fenceChar = null;
    let fenceLen = 0;
    const lines = md.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const lineStartPos = pos;
      pos += line.length + 1; // +1 for the split newline
      // Detect fenced code blocks
      const fenceOpen = line.match(/^([`~]{3,})/);
      if (!insideFence && fenceOpen) {
        insideFence = true; fenceChar = fenceOpen[1][0]; fenceLen = fenceOpen[1].length; 
        continue;
      }
      if (insideFence) {
        const fenceClose = new RegExp('^' + (fenceChar ? fenceChar : '`') + '{' + fenceLen + ',}\s*$');
        if (fenceClose.test(line)) { insideFence = false; fenceChar = null; fenceLen = 0; }
        continue;
      }
      // Headings only if not inside a fence
      const hm = line.match(/^(#{1,6})\s*(.*?)\s*#*\s*$/);
      if (!hm) continue;
      const hLevel = hm[1].length;
      const title = hm[2] || '';
      // Match [N] at start of the visible heading title
      const mId = title.match(/^\s*\[(\d+)\]/);
      if (mId && mId[1] === String(bugId) && hLevel === level) {
        start = lineStartPos;
        // Now find end: next heading with level <= current
        for (let j = i + 1, p = pos; j < lines.length; j++) {
          const ln = lines[j];
          const lnStart = p; p += ln.length + 1;
          const open2 = ln.match(/^([`~]{3,})/);
          if (!insideFence && open2) { // enter fence
            insideFence = true; fenceChar = open2[1][0]; fenceLen = open2[1].length; continue; }
          if (insideFence) {
            const fenceClose2 = new RegExp('^' + (fenceChar ? fenceChar : '`') + '{' + fenceLen + ',}\s*$');
            if (fenceClose2.test(ln)) { insideFence = false; fenceChar = null; fenceLen = 0; }
            continue;
          }
          const hm2 = ln.match(/^(#{1,6})\s*(.*?)\s*#*\s*$/);
          if (hm2) {
            const lvl2 = hm2[1].length;
            if (lvl2 <= level) { end = lnStart; break; }
          }
        }
        break;
      }
    }
    if (start === -1) return '';
    return md.slice(start, end).replace(/\s+$/, '') + '\n';
  }

  function getRenderedSectionText(headingEl) {
    if (!headingEl) return '';
    const parts = [];
    parts.push(headingEl.textContent || '');
    let node = headingEl.nextSibling;
    while (node) {
      if (node.nodeType === 1) {
        const tag = node.tagName ? node.tagName.toLowerCase() : '';
        if (/^h[1-6]$/.test(tag)) break;
        if (node.classList && node.classList.contains('ticket-box')) {
          node = node.nextSibling;
          continue;
        }
        if (tag === 'button' || tag === 'input' || tag === 'select' || tag === 'textarea') {
          node = node.nextSibling;
          continue;
        }
        if (tag === 'pre') {
          const code = node.querySelector('code');
          const codeText = code ? code.textContent || '' : node.textContent || '';
          const trimmed = codeText.trimEnd();
          if (trimmed) parts.push(trimmed);
          node = node.nextSibling;
          continue;
        }
        const text = (node.innerText || node.textContent || '').trim();
        if (text) parts.push(text);
      } else if (node.nodeType === 3) {
        const t = (node.textContent || '').trim();
        if (t) parts.push(t);
      }
      node = node.nextSibling;
    }
    return parts.join('\n\n').trim();
  }

  function enhanceCallouts() {
    const quotes = contentEl.querySelectorAll('blockquote');
    quotes.forEach(q => {
      const first = q.querySelector('p');
      const titleText = (first && first.textContent ? first.textContent.trim() : '') || '';
      if (/^(sources|relevant|note|tip|info|warning)/i.test(titleText)) {
        q.classList.add('callout');
        const header = document.createElement('div');
        header.className = 'callout-header';
        const icon = document.createElement('span');
        icon.setAttribute('aria-hidden', 'true');
        icon.textContent = '▸';
        const title = document.createElement('span');
        title.textContent = titleText.replace(/^\w+\s*:\s*/i, (m) => m.replace(/:\s*$/,'')).replace(/:$/, '');
        header.appendChild(icon);
        header.appendChild(title);

        const body = document.createElement('div');
        body.className = 'callout-body';
        const rest = Array.from(q.childNodes);
        q.innerHTML = '';
        q.appendChild(header);
        rest.forEach((node, idx) => {
          if (idx === 0 && node.nodeName === 'P') return; // skip first p (we used as header)
          body.appendChild(node);
        });
        q.appendChild(body);
        q.classList.add('open');
        header.addEventListener('click', () => {
          const isOpen = q.classList.toggle('open');
          icon.textContent = isOpen ? '▾' : '▸';
        });
        icon.textContent = '▾';
      }
    });
  }

  function enhanceMermaid() {
    if (!window.mermaid) return;
    // Convert fenced code blocks into .mermaid containers
    contentEl.querySelectorAll('pre > code.language-mermaid').forEach(code => {
      const parent = code.closest('pre');
      if (!parent) return;
      const div = document.createElement('div');
      div.className = 'mermaid';
      div.textContent = code.textContent || '';
      parent.replaceWith(div);
    });

    const theme = document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'default';
    const rs = getComputedStyle(document.documentElement);
    const sevHigh = (rs.getPropertyValue('--sev-high') || '#dc2626').trim();
    const sevMed  = (rs.getPropertyValue('--sev-med')  || '#eab308').trim();
    const sevLow  = (rs.getPropertyValue('--sev-low')  || '#059669').trim();
    const sevIgn  = (rs.getPropertyValue('--sev-ign')  || '#64748b').trim();
    // For pie charts we want 'ignore' to be white regardless of badge color
    const sevIgnPie = '#ffffff';
    const borderCol = (rs.getPropertyValue('--border') || '#e5e7eb').trim();
    // Neutral fill for unknown/unscored slices (use grey tone)
    const neutralFill = (rs.getPropertyValue('--sev-ign') || '#94a3b8').trim();
    const accent  = (rs.getPropertyValue('--accent')    || '#2563eb').trim();
    const textCol = (rs.getPropertyValue('--text')      || '#1f2937').trim();
    try {
      // Reset previously processed diagrams to allow re-render
      contentEl.querySelectorAll('.mermaid').forEach(el => {
        const element = el;
        const code = element.getAttribute('data-code') || element.textContent || '';
        const norm = getNormalizedPie(code);
        element.setAttribute('data-code', norm.code);
        if (norm.labels && norm.labels.length) {
          element.setAttribute('data-slices', JSON.stringify(norm.labels));
        } else {
          element.removeAttribute('data-slices');
        }
        element.removeAttribute('data-processed');
        element.textContent = norm.code;
      });
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: 'loose',
        theme,
        themeVariables: {
          pie1: sevHigh,
          pie2: sevMed,
          pie3: sevLow,
          pie4: sevIgnPie,
          pie5: accent,
          pie6: textCol,
        },
      });
      mermaid.run({ querySelector: '.mermaid' }).then(() => {
        // After render, enforce severity palette on pie charts by label
        document.querySelectorAll('#content .mermaid svg').forEach(svg => {
          const container = svg.closest('.mermaid');
          let labels = [];
          try {
            const attr = container && container.getAttribute('data-slices');
            if (attr) labels = JSON.parse(attr);
          } catch {}
          applySeverityColorsToPie(svg, { sevHigh, sevMed, sevLow, sevIgn: sevIgnPie, accent, textCol, borderCol, neutralFill }, labels);
      });
      });
    } catch (e) {
      // ignore
    }
  }

  function getNormalizedPie(code) {
    try {
      const lines = (code || '').split(/\r?\n/);
      // find first non-empty meaningful line
      const firstNonEmpty = lines.findIndex(l => l.trim().length > 0);
      if (firstNonEmpty === -1) return { code };
      if (!/^\s*pie\b/i.test(lines[firstNonEmpty])) return { code }; // not a pie chart

      const severityOrder = ['high', 'medium', 'low', 'ignore'];
      const titleLines = [];
      const sliceLines = [];
      const otherLines = [];
      for (let i = firstNonEmpty + 1; i < lines.length; i++) {
        const raw = lines[i];
        const t = raw.trim();
        if (t.length === 0) { otherLines.push(raw); continue; }
        if (/^title\b/i.test(t)) { titleLines.push(raw); continue; }
        // Match slice: label : value
        const m = t.match(/^((?:"[^"]+"|'[^']+'|[^:]+))\s*:\s*([0-9]+(?:\.[0-9]+)?)\s*$/);
        if (m) {
          // Extract label without quotes
          let label = m[1].trim();
          if ((label.startsWith('"') && label.endsWith('"')) || (label.startsWith("'") && label.endsWith("'"))) {
            label = label.slice(1, -1);
          }
          const value = m[2];
          sliceLines.push({ raw: raw, label, value });
        } else {
          otherLines.push(raw);
        }
      }
      if (sliceLines.length === 0) return { code };

      function sevFromLabel(lbl) {
        const L = (lbl || '').toLowerCase();
        if (L.includes('high')) return 'high';
        if (L.includes('medium') || L.includes('med')) return 'medium';
        if (L.includes('low')) return 'low';
        if (L.includes('ignore') || L.includes('ignored')) return 'ignore';
        return null;
      }

      const sevGroups = { high: [], medium: [], low: [], ignore: [] };
      const others = [];
      sliceLines.forEach(s => {
        const sev = sevFromLabel(s.label);
        if (sev && sevGroups[sev]) sevGroups[sev].push(s);
        else others.push(s);
      });

      // Rebuild code with pie line, title lines, then slices in severity order, then others
      const indent = '    ';
      const out = [];
      out.push(lines[firstNonEmpty]);
      titleLines.forEach(l => out.push(l));
      const labels = [];
      severityOrder.forEach(sev => {
        sevGroups[sev].forEach(s => {
          const label = /\s/.test(s.label) ? '"' + s.label + '"' : s.label;
          out.push(`${indent}${label}: ${s.value}`);
          labels.push(s.label);
        });
      });
      others.forEach(s => {
        const label = /\s/.test(s.label) ? '"' + s.label + '"' : s.label;
        out.push(`${indent}${label}: ${s.value}`);
        labels.push(s.label);
      });
      return { code: out.join('\n'), labels };
    } catch (e) {
      return { code };
    }
  }

  function applySeverityColorsToPie(svg, palette, labelsOrder) {
    try {
      // Locate legend items across Mermaid variants
      const legendItems = Array.from(
        svg.querySelectorAll('g.legend, g.legend > g, g[class*="legend"] > g, g[class*="legend"] g.legend-item')
      );
      // Try to find arcs (paths using arc command 'A')
      let arcPaths = Array.from(svg.querySelectorAll('path.pieCircle, path')).filter(p => {
        const d = (p.getAttribute('d') || '').toUpperCase();
        return d.includes('A') && /M\s*[-\d.]+\s*[,\s]\s*[-\d.]+/i.test(d);
      });

      const normalize = (s) => (s || '').trim().toLowerCase();
      const mapColor = (label) => {
        const l = normalize(label);
        // Match severity at the start of the label (case-insensitive),
        // allowing counts or extra text after, e.g., "High (12)".
        if (/^high\b/.test(l)) return palette.sevHigh;
        if (/^(medium|med)\b/.test(l)) return palette.sevMed;
        if (/^low\b/.test(l)) return palette.sevLow;
        if (/^ign(?:ore|ored)?\b/.test(l)) return palette.sevIgn;
        if (l.includes('unknown') || l.includes('unscored') || l.includes('unrated') || l.includes('unassigned')) return (palette.neutralFill || null);
        return null;
      };

      // Primary: recolor arcs by their own slice label (text within same group), excluding legend labels.
      (function colorArcsBySliceLabel(){
        try {
          const sliceGroups = Array.from(svg.querySelectorAll('g'))
            .filter(g => g.querySelector('path') && g.querySelector('text') && !g.closest('g[class*="legend"]'));
          let colored = 0;
          sliceGroups.forEach(g => {
            const labelEl = g.querySelector('text');
            const path = g.querySelector('path');
            if (!labelEl || !path) return;
            const label = labelEl.textContent || '';
            const color = mapColor(label);
            if (!color) return;
            path.setAttribute('fill', color);
            const l = normalize(label);
            if (/^ign(?:ore|ored)\b/.test(l)) {
              path.setAttribute('stroke', palette.borderCol || '#e5e7eb');
              path.setAttribute('stroke-width', '1');
            } else {
              path.removeAttribute('stroke');
              path.removeAttribute('stroke-width');
            }
            colored++;
          });
          if (colored) return; // stop if arcs colored via slice labels
        } catch {}
        // Fallback: recolor arcs by legend order (assumes legend order follows arc order)
        if (legendItems.length && arcPaths.length) {
          const legendLabels = legendItems.map((item) => {
            const t = item.querySelector('text');
            return t ? (t.textContent || '') : '';
          });
          legendLabels.forEach((label, idx) => {
            const color = mapColor(label);
            const path = arcPaths[idx];
            if (!path || !color) return;
            path.setAttribute('fill', color);
            const l = normalize(label);
            if (/^ign(?:ore|ored)\b/.test(l)) {
              path.setAttribute('stroke', palette.borderCol || '#e5e7eb');
              path.setAttribute('stroke-width', '1');
            } else {
              path.removeAttribute('stroke');
              path.removeAttribute('stroke-width');
            }
          });
        }
      })();

      // Color legend markers by their text labels (robust to DOM ordering)
      if (legendItems.length) {
        legendItems.forEach((item) => {
          const textEl = item.querySelector('text');
          const label = textEl ? textEl.textContent : '';
          const color = mapColor(label);
          const marker = item.querySelector('rect, path, circle');
          if (marker) {
            const l = normalize(label);
            if (color) {
              // Update inline style if present (takes precedence over attributes)
              const style = (marker.getAttribute('style') || '').trim();
              if (style) {
                // Replace existing fill and stroke entries; preserve others
                let next = style
                  .replace(/fill\s*:\s*[^;]+/i, '')
                  .replace(/stroke\s*:\s*[^;]+/i, '')
                  .replace(/;;+/g, ';')
                  .replace(/^;|;$/g, '');
                next = (next ? next + '; ' : '') + `fill: ${color}`;
                if (/^ign(?:ore|ored)\b/.test(l)) {
                  next += `; stroke: ${palette.borderCol || '#e5e7eb'}`;
                }
                marker.setAttribute('style', next);
              } else {
                marker.setAttribute('fill', color);
                if (/^ign(?:ore|ored)\b/.test(l)) {
                  marker.setAttribute('stroke', palette.borderCol || '#e5e7eb');
                  marker.setAttribute('stroke-width', '1');
                } else {
                  marker.removeAttribute('stroke');
                  marker.removeAttribute('stroke-width');
                }
              }
            } else if (palette.neutralFill && (l.includes('unknown') || l.includes('unscored') || l.includes('unrated') || l.includes('unassigned'))) {
              const style = (marker.getAttribute('style') || '').trim();
              if (style) {
                let next = style
                  .replace(/fill\s*:\s*[^;]+/i, '')
                  .replace(/stroke\s*:\s*[^;]+/i, '')
                  .replace(/;;+/g, ';')
                  .replace(/^;|;$/g, '');
                next = (next ? next + '; ' : '') + `fill: ${palette.neutralFill}` + `; stroke: ${palette.borderCol || '#e5e7eb'}`;
                marker.setAttribute('style', next);
              } else {
                marker.setAttribute('fill', palette.neutralFill);
                marker.setAttribute('stroke', palette.borderCol || '#e5e7eb');
                marker.setAttribute('stroke-width', '1');
              }
            }
          }
        });

        // Reorder legend visually and in DOM as High -> Medium -> Low -> Ignore -> Others
        try {
          const sevKey = (label) => {
            const l = normalize(label);
            if (/^high\b/.test(l)) return 'high';
            if (/^(medium|med)\b/.test(l)) return 'medium';
            if (/^low\b/.test(l)) return 'low';
            if (/^ign(?:ore|ored)\b/.test(l)) return 'ignore';
            return 'other';
          };
          const pri = { high: 0, medium: 1, low: 2, ignore: 3, other: 4 };
          const items = legendItems.map((g) => {
            const tx = (g.getAttribute('transform') || '').match(/translate\(([^,]+),\s*([^\)]+)\)/i);
            const x = tx ? parseFloat(tx[1]) : 0;
            const y = tx ? parseFloat(tx[2]) : 0;
            const textEl = g.querySelector('text');
            const label = textEl ? (textEl.textContent || '') : '';
            return { g, x, y, label, key: sevKey(label) };
          });
          if (items.length) {
            const parent = items[0].g.parentNode;
            const ys = items.map(i => i.y).sort((a,b) => a - b);
            const step = ys.length >= 2 ? Math.max(18, Math.round(ys[1] - ys[0])) : 22;
            const baseX = items.reduce((acc, i) => isNaN(i.x) ? acc : i.x, items[0].x || 0);
            const baseY = Math.min.apply(null, ys);
            const sorted = items.slice().sort((a,b) => {
              const da = pri[a.key] ?? 4;
              const db = pri[b.key] ?? 4;
              if (da !== db) return da - db;
              // Stable secondary by original Y to keep consistent spacing within same group
              return a.y - b.y;
            });
            sorted.forEach((it, idx) => {
              const y = baseY + idx * step;
              it.g.setAttribute('transform', `translate(${baseX},${y})`);
              // Move node to reflect sorted order in DOM
              try { parent.appendChild(it.g); } catch {}
            });
          }
        } catch {}
      }
    } catch (e) {
      // no-op
    }
  }

  // Determine if a heading's section has any substantive content until the next heading of same or higher level
function isSectionEmpty(headingEl, depth) {
    if (!headingEl) return true;
    const stopDepth = depth; // stop at next heading with depth <= this
    let node = headingEl.nextSibling;
    while (node) {
      if (node.nodeType === 1) { // element
        const el = node;
        const tag = el.tagName ? el.tagName.toLowerCase() : '';
    if (/^h[1-6]$/.test(tag)) {
          const d = (function(t){ try { return parseInt(t.replace(/[^0-9]/g,'') || '0', 10); } catch { return 0; } })(tag);
          if (d <= stopDepth) return true; // reached next section without content
        } else {
          const text = (el.textContent || '').trim();
          const isMermaid = el.classList && el.classList.contains('mermaid');
          if (isMermaid) return false;
          if (el.tagName === 'UL' || el.tagName === 'OL') {
            if (el.querySelector('li')) return false;
          }
          if (el.tagName === 'TABLE' || el.tagName === 'PRE' || el.tagName === 'BLOCKQUOTE' || el.tagName === 'P' || el.tagName === 'IMG' || el.tagName === 'FIGURE' || el.tagName === 'DETAILS' || el.tagName === 'DIV') {
            if (text.length > 0) return false;
          }
        }
      } else if (node.nodeType === 3) { // text node
        if ((node.textContent || '').trim().length > 0) return false;
      }
      node = node.nextSibling;
    }
    return true;
  }

  let includedSectionIds = new Set();
  let headingIndex = [];
  function buildNavigation(headings) {
    const items = headings
      .map(h => ({
        id: h.id,
        text: h.textContent || '',
        depth: (function(tag){ try { return parseInt(tag.replace(/[^0-9]/g,'') || '0', 10); } catch { return 0; } })(h.tagName || ''),
      }))
      .filter(i => (i.text || '').trim().length > 0)
      .filter(i => !isSectionEmpty(document.getElementById(i.id), i.depth));
    // Attach section content snapshot for search scoring
    headingIndex = items.map(i => ({
      ...i,
      content: (getSectionTextById(i.id, i.depth, 4000) || ''),
    }));
    // Initial render (will show full list if search is empty)
    renderFilteredNavigation(sectionSearch ? sectionSearch.value : '');
  }

function getSectionTextById(id, depth, maxChars) {
    const el = document.getElementById(id);
    if (!el) return '';
    const stopDepth = depth;
    let text = '';
    let node = el.nextSibling;
    while (node && text.length < (maxChars || 4000)) {
      if (node.nodeType === 1) {
        const tag = node.tagName ? node.tagName.toLowerCase() : '';
        if (/^h[1-6]$/.test(tag)) {
          const d = (function(t){ try { return parseInt(t.replace(/[^0-9]/g,'') || '0', 10); } catch { return 0; } })(tag);
          if (d <= stopDepth) break;
        } else {
          text += ' ' + (node.innerText || node.textContent || '');
        }
      } else if (node.nodeType === 3) {
        text += ' ' + (node.textContent || '');
      }
      node = node.nextSibling;
    }
    return text.trim();
  }

  function updateSectionSearch() {
    // Re-render the TOC according to the current query
    renderFilteredNavigation(sectionSearch ? sectionSearch.value : '');
  }

  function handleSectionSearch(q) {
    const query = (q || '').trim();
    if (!query) return;
    const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (headingIndex.length === 0) return;

    let best = null;
    let bestScore = -Infinity;
    headingIndex.forEach((item, idx) => {
      const text = (item.text || '').toLowerCase();
      const content = (item.content || '').toLowerCase();
      let score = 0;
      if (text === query.toLowerCase()) score += 100;
      if (text.startsWith(query.toLowerCase())) score += 50;
      if (tokens.length && tokens.every(t => text.includes(t))) score += 10 * tokens.length;
      // Content-based scoring
      if (tokens.length && tokens.every(t => content.includes(t))) score += 6 * tokens.length;
      // prefer shallower depth slightly
      score += (7 - item.depth);
      // slight bias to earlier items
      score -= idx * 0.001;
      if (score > bestScore) { bestScore = score; best = item; }
    });
    if (best) {
      const el = document.getElementById(best.id);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
        try { history.replaceState(null, '', `#${best.id}`); } catch {}
        try { sectionSearch.value = ''; } catch {}
      }
    }
  }

  // Render the TOC list filtered by the query; empty query shows all
  function renderFilteredNavigation(query) {
    const q = (query || '').trim().toLowerCase();
    let items = headingIndex.slice();
    if (q) {
      const tokens = q.split(/\s+/).filter(Boolean);
      const scored = items.map((item, idx) => {
        const text = (item.text || '').toLowerCase();
        const content = (item.content || '').toLowerCase();
        let score = 0;
        if (text === q) score += 100;
        if (text.startsWith(q)) score += 50;
        if (tokens.length && tokens.every(t => text.includes(t))) score += 10 * tokens.length;
        if (tokens.length && tokens.every(t => content.includes(t))) score += 6 * tokens.length;
        score += (7 - item.depth);
        score -= idx * 0.001;
        return { item, score };
      });
      items = scored.filter(s => s.score > 0).sort((a,b) => b.score - a.score).map(s => s.item);
    }

    tocList.innerHTML = '';
    const tocUl = document.createElement('ul');
    items.forEach(i => {
      const li = document.createElement('li');
      li.className = `toc-item depth-${i.depth}`;
      const a = document.createElement('a');
      a.href = `#${i.id}`;
      a.textContent = i.text;
      a.addEventListener('click', (e) => {
        e.preventDefault();
        var target = document.getElementById(i.id);
        if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
        history.replaceState(null, '', `#${i.id}`);
      });
      li.appendChild(a);
      tocUl.appendChild(li);
    });
    tocList.appendChild(tocUl);
    includedSectionIds = new Set(items.map(i => i.id));
    initScrollSpy();
  }

  let spyObserver;
  function initScrollSpy() {
    if (spyObserver) spyObserver.disconnect();

    const sections = Array.from(contentEl.querySelectorAll('h1, h2, h3, h4, h5, h6')).filter(sec => includedSectionIds.has(sec.id));
    const map = new Map();
    sections.forEach(sec => {
      const id = sec.id;
      if (!id) return;
      map.set(id, {
        toc: (function(){
          const a = tocList.querySelector('a[href="#' + CSS.escape(id) + '"]');
          return a ? a.parentElement : null;
        })(),
      });
    });

    const setActive = (id) => {
      tocList.querySelectorAll('.active').forEach(el => el.classList.remove('active'));
      const pair = map.get(id);
      if (pair && pair.toc) {
        pair.toc.classList.add('active');
        const container = tocList.closest('.toc-inner') || tocList.parentElement;
        scrollIntoViewIfNeeded(pair.toc, container);
      }
    };

    spyObserver = new IntersectionObserver((entries) => {
      let topMost = null;
      entries.forEach(ent => {
        if (!ent.isIntersecting) return;
        if (!topMost || ent.boundingClientRect.top < topMost.boundingClientRect.top) {
          topMost = ent;
        }
      });
      if (topMost && topMost.target && topMost.target.id) setActive(topMost.target.id);
    }, { rootMargin: '-56px 0px -70% 0px', threshold: [0, 1] });

    sections.forEach(sec => spyObserver.observe(sec));
  }

  function scrollIntoViewIfNeeded(target, container) {
    if (!target || !container) return;
    try {
      const cRect = container.getBoundingClientRect();
      const tRect = target.getBoundingClientRect();
      const margin = 12;
      const overTop = tRect.top < cRect.top + margin;
      const overBottom = tRect.bottom > cRect.bottom - margin;
      if (overTop) {
        container.scrollTop -= (cRect.top + margin - tRect.top);
      } else if (overBottom) {
        container.scrollTop += (tRect.bottom - (cRect.bottom - margin));
      }
    } catch (e) { /* noop */ }
  }

  function basicMarkdown(md) {
    const esc = (s) => s.replace(/[&<>]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));
    const formatInline = (s) => {
      let t = esc(s);
      // 1) Inline code first to protect spans from other regexes
      //    Require whitespace/punctuation boundaries to avoid mid-word false positives
      t = t.replace(/(^|\s)`([^`]+)`(?=\s|$|[.,;:!\?\)\]\}])/g, '$1<code>$2</code>');
      // 2) Links [text](url)
      t = t.replace(/\[([^\]]+)\]\(([^\)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
      // 3) Autolinks (avoid matching inside tags or code/backticks); don't eat trailing backticks or ')'
      t = t.replace(/(?<![\"'=`])(https?:\/\/[^\s`)]+)(?![^<]*>)/g, '<a href="$1" target="_blank" rel="noopener noreferrer">$1</a>');
      // 4) Strikethrough ~~text~~
      t = t.replace(/~~([^~]+)~~/g, '<del>$1</del>');
      // 5) Bold **text**
      t = t.replace(/\*\*([^*]+)\*\*/g, '<strong>$1<\/strong>');
      // 6) Italics *text*
      t = t.replace(/(^|\W)\*([^*]+)\*(?=\W|$)/g, '$1<em>$2<\/em>');
      return t;
    };
    const lines = md.split(/\r?\n/);
    let html = '';
    let inCode = false;
    let codeLang = '';
    let fenceType = '```';
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const fence = line.match(/^\s*(```|~~~)\s*(.*)$/);
      if (fence) {
        if (!inCode) {
          fenceType = fence[1];
          const info = (fence[2] || '').trim();
          codeLang = (info.split(/\s+/)[0] || '').toLowerCase();
          const safeLang = codeLang.replace(/[^a-z0-9_+\-]/g, '');
          if (codeLang === 'mermaid') {
            html += '<div class="mermaid">';
          } else {
            const cls = safeLang ? ` class="language-${safeLang}"` : '';
            html += `<pre><code${cls}>`;
          }
          inCode = true;
        } else {
          if (codeLang === 'mermaid') {
            html += '</div>';
          } else {
            html += '</code></pre>';
          }
          inCode = false;
          codeLang = '';
        }
        continue;
      }
      if (inCode) { html += esc(line) + '\n'; continue; }

      // Setext headings
      if (i + 1 < lines.length && (/^===+\s*$/.test(lines[i + 1]) || /^---+\s*$/.test(lines[i + 1]))) {
        const lvl = /^===+\s*$/.test(lines[i + 1]) ? 1 : 2;
        html += `<h${lvl}>${esc(line.trim())}</h${lvl}>`;
        i += 1; // skip underline
        continue;
      }

      if (/^#\s+/.test(line)) { html += `<h1>${formatInline(line.replace(/^#\s+/, ''))}</h1>`; continue; }
      if (/^##\s+/.test(line)) { html += `<h2>${formatInline(line.replace(/^##\s+/, ''))}</h2>`; continue; }
      if (/^###\s+/.test(line)) { html += `<h3>${formatInline(line.replace(/^###\s+/, ''))}</h3>`; continue; }
      if (/^####\s+/.test(line)) { html += `<h4>${formatInline(line.replace(/^####\s+/, ''))}</h4>`; continue; }
      if (/^#####\s+/.test(line)) { html += `<h5>${formatInline(line.replace(/^#####\s+/, ''))}</h5>`; continue; }
      if (/^######\s+/.test(line)) { html += `<h6>${formatInline(line.replace(/^######\s+/, ''))}</h6>`; continue; }
      if (/^>\s?/.test(line)) { html += `<blockquote><p>${formatInline(line.replace(/^>\s?/, ''))}</p></blockquote>`; continue; }
      if (line.trim() === '') { html += ''; continue; }

      // Simple tables (GFM)
      if (line.includes('|') && i + 1 < lines.length && /^\s*\|?\s*:?[-\s]+:?\s*\|/.test(lines[i + 1])) {
        // collect block until a non-row line
        const header = line.trim();
        const divider = lines[i + 1].trim();
        i += 2;
        const rows = [];
        while (i < lines.length && lines[i].includes('|') && !/^\s*$/.test(lines[i])) {
          rows.push(lines[i].trim());
          i++;
        }
        i--; // step back one because loop will i++
        const cells = (row) => row.replace(/^\|?|\|?$/g, '').split('|').map(s => s.trim());
        const ths = cells(header).map(h => `<th>${formatInline(h)}</th>`).join('');
        const tds = rows.map(r => `<tr>${cells(r).map(c => `<td>${formatInline(c)}</td>`).join('')}</tr>`).join('');
        html += `<table><thead><tr>${ths}</tr></thead><tbody>${tds}</tbody></table>`;
        continue;
      }

      // Lists (unordered/ordered + task lists)
      const ulMatch = line.match(/^\s*([*+-])\s+(.+)/);
      const olMatch = line.match(/^\s*(\d+)\.\s+(.+)/);
      if (ulMatch || olMatch) {
        const isOrdered = !!olMatch;
        const tag = isOrdered ? 'ol' : 'ul';
        const items = [];
        let isTaskList = false;
        // collect contiguous list lines
        while (i < lines.length) {
          const l = lines[i];
          const m = isOrdered ? l.match(/^\s*\d+\.\s+(.+)/) : l.match(/^\s*([*+-])\s+(.+)/);
          if (!m) break;
          let text = isOrdered ? m[1] : m[2];
          const task = text.match(/^\[( |x|X)\]\s+(.+)/);
          if (task) {
            isTaskList = true;
            const checked = /x/i.test(task[1]);
            items.push({ task: true, checked, text: task[2] });
          } else {
            items.push({ task: false, text });
          }
          i++;
        }
        i--; // for loop increment
        if (isTaskList && !isOrdered) {
          html += `<ul class="task-list">`;
          items.forEach(it => {
            const id = 'cb-' + Math.random().toString(36).slice(2);
            html += `<li class="task-list-item"><input id="${id}" type="checkbox" ${it.checked ? 'checked' : ''} disabled><label for="${id}">${formatInline(it.text)}</label></li>`;
          });
          html += `</ul>`;
        } else {
          html += `<${tag}>`;
          items.forEach(it => {
            html += `<li>${formatInline(it.text)}</li>`;
          });
          html += `</${tag}>`;
        }
        continue;
      }

      // Paragraph with inline formatting
      html += `<p>${formatInline(line)}</p>`;
    }
    return html;
  }

  (async function bootstrap() {
    // Initialize theme before rendering anything
    try { initTheme(); } catch {}
    // Initialize sidebar collapsed state
    try {
      const saved = localStorage.getItem('sidebar:collapsed');
      if (saved === '1') setSidebarCollapsed(true); else setSidebarCollapsed(false);
    } catch { setSidebarCollapsed(false); }
    // If ?file= is present, try to load from sessionStorage first, else fetch relative URL
    const urlObj = new URL(location.href);
    // Fullscreen viewer mode
    const full = urlObj.searchParams.get('full');
    if (full && /^(1|true)$/i.test(full)) {
      document.body.classList.add('fullviewer');
    }
    const fileParam = urlObj.searchParams.get('file');
    const remoteParam = urlObj.searchParams.get('remote');
    if (remoteParam) {
      try {
        const resp = await fetch('/api/file?path=' + encodeURIComponent(remoteParam));
        if (!resp.ok) throw new Error('HTTP ' + resp.status);
        const txt = await resp.text();
        renderMarkdown(txt);
        sitePath.textContent = `/ Remote / ${remoteParam}`;
        return;
      } catch (e) {}
    }
    if (fileParam) {
      const fileName = decodeURIComponent(fileParam);
      try {
        const cached = sessionStorage.getItem('doc:' + fileName);
        if (cached) {
          renderMarkdown(cached);
          sitePath.textContent = `/ Docs / ${fileName}`;
          return;
        }
      } catch {}
      const loaded = await fetchAndRender(fileName, null);
      if (loaded) return;
    }

    // Prefer pre-bundled JS payload if available (works over file:// without fetch)
    if (typeof window.REPORT_MD === 'string' && window.REPORT_MD.length) {
      renderMarkdown(window.REPORT_MD);
      sitePath.textContent = '/ Report / report.md';
      return;
    }
    // Try to auto-load report.md if present via fetch (served over http)
    const loadedReport = await fetchAndRender('report.md', null);
    if (loadedReport) return;

    // Fallback demo content from prompt.txt
    try {
      const demo = await fetch('prompt.txt').then(r => r.text());
      const md = `# Markdown Viewer\n\nUpload a Markdown file (drag & drop or use Open) to see it rendered with navigation and a table of contents.\n\n> Note: This page uses your local file only; nothing is uploaded.\n\n## Getting started\n\n- Click Open or drag a .md file into the window.\n- Navigate using the left sidebar or right outline.\n\n## Features\n\n${'```'}js\nconsole.log('Code blocks support copy button and syntax highlighting');\n${'```'}\n\n${'```'}mermaid\nflowchart LR\n  A[Drag file] --> B{Parse Markdown}\n  B -->|Build| C[Nav/ToC]\n  B -->|Render| D[Content]\n  C --> E[Scroll Spy]\n${'```'}\n\n| Feature | Status |\n|---|---|\n| Drag & Drop | ✅ |\n| Scroll Spy | ✅ |\n| Dark Mode | ✅ |\n\n> Sources: \n> styles.css\n> script.js\n> index.html\n`;
      renderMarkdown(md);
    } catch {
      renderMarkdown('# Markdown Viewer\n\nOpen a local Markdown file to begin.');
    }
  })();

  window.addEventListener('hashchange', () => {
    const id = decodeURIComponent(location.hash.replace('#', ''));
    const el = document.getElementById(id);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  });

  // Auto-refresh viewer if a job updates the report in another tab
  window.addEventListener('storage', (e) => {
    if (e.key === 'report:updated') {
      // Re-fetch report.md and render
      fetchAndRender('report.md', null);
    }
  });

  // Floating ChatGPT widget behavior
  (function initChatWidget(){
    const toggle = document.getElementById('chatToggle');
    const panel = document.getElementById('chatPanel');
    const close = document.getElementById('chatClose');
    const copyBtn = document.getElementById('copyPromptBtn');
    const openBtn = document.getElementById('openChatBtn');
    const input = document.getElementById('chatInput');
    const modelSel = document.getElementById('chatModel');
    const includeCbx = document.getElementById('includeContext');
    const status = document.getElementById('chatStatus');
    if (!toggle || !panel) return;

    function setOpen(open){
      panel.hidden = !open;
      toggle.setAttribute('aria-expanded', String(open));
      if (open) {
        // Prefill prompt with current section context if empty
        if (input && !input.value.trim()) {
          const ctx = currentSectionContext();
          input.value = ctx ? ctx + '\n\n' : '';
          if (includeCbx) includeCbx.checked = false; // avoid duplicating context
        }
        if (input) {
          try { input.focus(); input.setSelectionRange(input.value.length, input.value.length); } catch {}
        }
      }
    }
    toggle.addEventListener('click', (e)=> { e.stopPropagation(); setOpen(panel.hidden); });
    close && close.addEventListener('click', ()=> setOpen(false));

    // Close on outside click / ESC
    document.addEventListener('click', (e)=>{
      if (panel.hidden) return;
      if (!panel.contains(e.target) && !toggle.contains(e.target)) setOpen(false);
    });
    document.addEventListener('keydown', (e)=>{
      if (e.key === 'Escape') setOpen(false);
    });

    function currentSectionContext(maxChars=2000){
      // find active heading (h2/h3 preferred, else h1)
      const active = (function(){
        const a = document.querySelector('#tocList .active a');
        if (a) return document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
        return contentEl.querySelector('h2, h3, h1');
      })();
      if (!active) return '';
      let ctx = [];
      let n = active.nextSibling;
      while (n && ctx.join('\n').length < maxChars){
        if (n.nodeType === 1){
          const tag = n.tagName.toLowerCase();
          if (/^h[1-6]$/.test(tag)) break;
          ctx.push(n.innerText || '');
        } else if (n.nodeType === 3) {
          ctx.push(n.textContent || '');
        }
        n = n.nextSibling;
      }
      const heading = (active.textContent || '').trim();
      return `Context: ${heading}\n\n` + ctx.join('\n').trim().slice(0, maxChars);
    }

    function buildPrompt(){
      const q = (input && input.value.trim()) || '';
      const model = (modelSel && modelSel.value) || 'gpt-4o';
      const include = includeCbx && includeCbx.checked;
      const parts = [];
      parts.push(`Question: ${q}`);
      parts.push(`Model: ${model}`);
      parts.push(`Source: ${location.href}`);
      if (include) parts.push(currentSectionContext());
      return parts.join('\n\n');
    }

    copyBtn && copyBtn.addEventListener('click', async ()=>{
      const prompt = buildPrompt();
      try {
        await navigator.clipboard.writeText(prompt);
        status.textContent = 'Prompt copied to clipboard';
        setTimeout(()=> status.textContent = '', 1500);
      } catch(e) {
        alert(prompt);
      }
    });

    openBtn && openBtn.addEventListener('click', async ()=>{
      const model = (modelSel && modelSel.value) || 'gpt-4o';
      const url = `https://chat.openai.com/?model=${encodeURIComponent(model)}`;
      const prompt = buildPrompt();
      try { await navigator.clipboard.writeText(prompt); } catch {}
      window.open(url, '_blank', 'noopener');
    });
  })();
})();
