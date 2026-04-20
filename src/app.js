// DPCrawler Frontend
document.addEventListener('DOMContentLoaded', function () {

  // ── Theme management ──
  function applyTheme(mode) {
    var resolved = mode;
    if (mode === 'auto') {
      resolved = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    }
    document.documentElement.setAttribute('data-theme', resolved);
  }

  var savedTheme = localStorage.getItem('dp-theme') || 'auto';
  applyTheme(savedTheme);

  // Listen for system theme changes when in auto mode
  window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', function () {
    if ((localStorage.getItem('dp-theme') || 'auto') === 'auto') {
      applyTheme('auto');
    }
  });

  var configPath = 'config.yaml';
  var isRunning = false;
  var crawledFiles = [];
  var crawlGeneration = 0; // guards against stale async callbacks resetting UI

  var preCrawlData = null; // stores pre-crawl result for progress estimation

  // Block reload (Ctrl+R, F5, Cmd+R) and close (Ctrl+Q) during crawl
  document.addEventListener('keydown', function (e) {
    if (!isRunning) return;
    if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key === 'r')) {
      e.preventDefault();
      e.stopPropagation();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'q') {
      e.preventDefault();
      e.stopPropagation();
    }
  });
  window.addEventListener('beforeunload', function (e) {
    if (isRunning) {
      e.preventDefault();
      e.returnValue = '';
    }
  });

  // Listen for close confirmation during crawl (triggered by Rust)
  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen('confirm-exit', function () {
      if (confirm('\u722c\u53d6\u6b63\u5728\u8fdb\u884c\u4e2d\uff0c\u786e\u5b9a\u8981\u9000\u51fa\u5417\uff1f')) {
        window.__TAURI__.core.invoke('force_quit');
      }
    });
  }

  var el = {
    urls: document.getElementById('urls'),
    extensions: document.getElementById('extensions'),
    outputDir: document.getElementById('outputDir'),
    browseDirBtn: document.getElementById('browseDirBtn'),
    openDirBtn: null,
    contentFormat: document.getElementById('contentFormat'),
    delay: document.getElementById('delay'),
    maxDepth: document.getElementById('maxDepth'),
    minYear: document.getElementById('minYear'),
    startBtn: document.getElementById('startBtn'),
    stopBtn: document.getElementById('stopBtn'),
    preCrawlBtn: document.getElementById('preCrawlBtn'),
    statusDot: document.getElementById('statusDot'),
    statusText: document.getElementById('statusText'),
    progressFill: document.getElementById('progressFill'),
    progressText: document.getElementById('progressText'),
    logContainer: document.getElementById('logContainer'),
    fileList: document.getElementById('fileList'),
    newCount: document.getElementById('newCount'),
    updatedCount: document.getElementById('updatedCount'),
    unchangedCount: document.getElementById('unchangedCount'),
    errorCount: document.getElementById('errorCount'),
    totalCount: document.getElementById('totalCount'),
    previewTitle: document.getElementById('previewTitle'),
    previewContent: document.getElementById('previewContent'),
    previewModeBtn: document.getElementById('previewModeBtn'),
    rawIframe: document.getElementById('rawIframe'),
    docxContainer: document.getElementById('docxContainer'),
    xlsxContainer: document.getElementById('xlsxContainer'),
    siteList: document.getElementById('siteList'),
    themeSelect: document.getElementById('themeSelect')
  };

  var activeSite = null; // currently selected site in sidebar

  // Event delegation for file list (single click = select, double click = open URL)
  el.fileList.addEventListener('click', function (e) {
    var item = e.target.closest('.file-item[data-index]');
    if (!item) return;
    var idx = parseInt(item.dataset.index, 10);
    var f = crawledFiles[idx];
    if (f) selectFileItem(item, f.name, f.url);
  });
  el.fileList.addEventListener('dblclick', function (e) {
    var item = e.target.closest('.file-item[data-index]');
    if (!item) return;
    var idx = parseInt(item.dataset.index, 10);
    var f = crawledFiles[idx];
    if (f && f.url) invoke('open_url', { url: f.url });
  });

  // Wire up theme selector
  el.themeSelect.value = savedTheme;
  el.themeSelect.addEventListener('change', function () {
    var mode = el.themeSelect.value;
    localStorage.setItem('dp-theme', mode);
    applyTheme(mode);
  });

  // Tab switching
  document.querySelectorAll('.tab').forEach(function (tab) {
    tab.addEventListener('click', function () {
      document.querySelectorAll('.tab').forEach(function (t) { t.classList.remove('active'); });
      document.querySelectorAll('.tab-content').forEach(function (c) { c.classList.remove('active'); });
      tab.classList.add('active');
      document.getElementById(tab.dataset.tab).classList.add('active');
    });
  });

  // Panel resizer drag
  (function () {
    var resizer = document.getElementById('panelResizer');
    var fileList = document.getElementById('fileList');
    if (!resizer || !fileList) return;
    var startX, startW;
    resizer.addEventListener('mousedown', function (e) {
      e.preventDefault();
      startX = e.clientX;
      startW = fileList.offsetWidth;
      resizer.classList.add('active');
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
    function onMove(e) {
      var w = startW + (e.clientX - startX);
      if (w >= 150 && w <= fileList.parentElement.offsetWidth * 0.6) {
        fileList.style.flex = '0 0 ' + w + 'px';
      }
    }
    function onUp() {
      resizer.classList.remove('active');
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    }
  })();

  var MAX_LOG_LINES = 1000;

  // Batched log rendering to prevent UI freeze from event flooding
  var _logQueue = [];
  var _logRafScheduled = false;

  function _flushLogQueue() {
    _logRafScheduled = false;
    if (_logQueue.length === 0) return;
    var frag = document.createDocumentFragment();
    for (var i = 0; i < _logQueue.length; i++) {
      frag.appendChild(_logQueue[i]);
    }
    _logQueue = [];
    el.logContainer.appendChild(frag);
    // Trim excess lines
    while (el.logContainer.childElementCount > MAX_LOG_LINES) {
      el.logContainer.removeChild(el.logContainer.firstChild);
    }
    el.logContainer.scrollTop = el.logContainer.scrollHeight;
  }

  function log(msg, type) {
    type = type || 'info';
    var span = document.createElement('span');
    span.className = 'log-' + type;
    span.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg + '\n';
    _logQueue.push(span);
    if (!_logRafScheduled) {
      _logRafScheduled = true;
      requestAnimationFrame(_flushLogQueue);
    }
  }

  // File type icon using inline SVGs — each category has a visually distinct shape
  var lucideSvg = {
    'pdf': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#e53e3e" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><text x="12" y="17" text-anchor="middle" stroke="none" fill="#e53e3e" font-size="7" font-weight="bold" font-family="sans-serif">PDF</text></svg>',
    'doc': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#2b6cb0" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M8 13h8"/><path d="M8 17h5"/><path d="M8 9h3"/></svg>',
    'xls': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#276749" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><rect x="7" y="12" width="10" height="7" rx="1"/><path d="M7 15.5h10"/><path d="M12 12v7"/></svg>',
    'ppt': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#c05621" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><rect x="7" y="11" width="10" height="7" rx="1.5"/><path d="M10 14h4"/></svg>',
    'archive': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#744210" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><rect x="10" y="6" width="4" height="2"/><rect x="10" y="10" width="4" height="2"/><rect x="10" y="14" width="4" height="2"/><rect x="10" y="18" width="4" height="2"/></svg>',
    'image': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#6b46c1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/></svg>',
    'music': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#d53f8c" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>',
    'video': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#e53e3e" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="14" height="12" rx="2"/><path d="m16 12 5.223-3.482A.5.5 0 0 1 22 8.934v6.132a.5.5 0 0 1-.777.416L16 12Z"/></svg>',
    'code': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#4a5568" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="m10 13-2 2 2 2"/><path d="m14 17 2-2-2-2"/></svg>',
    'globe': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#3182ce" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/></svg>',
    'txt': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#718096" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M8 13h8"/><path d="M8 17h8"/></svg>',
    'file': '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>'
  };

  var extToIcon = {
    'pdf': 'pdf',
    'doc': 'doc', 'docx': 'doc', 'rtf': 'doc', 'odt': 'doc',
    'xls': 'xls', 'xlsx': 'xls', 'csv': 'xls', 'ods': 'xls',
    'ppt': 'ppt', 'pptx': 'ppt', 'odp': 'ppt',
    'txt': 'txt', 'md': 'txt',
    'zip': 'archive', 'rar': 'archive', '7z': 'archive', 'gz': 'archive', 'tar': 'archive',
    'jpg': 'image', 'jpeg': 'image', 'png': 'image', 'gif': 'image', 'svg': 'image', 'bmp': 'image', 'webp': 'image',
    'mp3': 'music', 'wav': 'music',
    'mp4': 'video', 'avi': 'video', 'mov': 'video',
    'json': 'code', 'xml': 'code', 'yaml': 'code', 'yml': 'code',
    'html': 'globe'
  };

  function getFileTypeFromUrl(url) {
    if (!url) return 'html';
    try {
      var pathname = new URL(url).pathname;
      var dot = pathname.lastIndexOf('.');
      if (dot !== -1) {
        var ext = pathname.substring(dot + 1).toLowerCase();
        if (ext && ext.length <= 5 && extToIcon[ext]) return ext;
      }
    } catch (e) { }
    return 'html';
  }

  function fileTypeIconHtml(ext) {
    var iconName = extToIcon[ext] || 'file';
    return '<span class="file-type-icon">' + (lucideSvg[iconName] || lucideSvg['file']) + '</span>';
  }

  function addFileToList(name, status, url) {
    if (!name) return;
    crawledFiles.push({ name: name, status: status, url: url || '' });
    // Cap file list to avoid DOM bloat
    if (crawledFiles.length <= 2000) {
      appendFileItem(crawledFiles[crawledFiles.length - 1], crawledFiles.length - 1);
    }
  }

  function appendFileItem(f, index) {
    var badgeClass = f.status === 'new' ? 'new' : (f.status === 'updated' ? 'updated' : (f.status === 'unchanged' ? 'unchanged' : ''));
    var badgeText = f.status === 'new' ? '\u65b0\u589e' : (f.status === 'updated' ? '\u66f4\u65b0' : '\u672a\u53d8');
    var div = document.createElement('div');
    div.className = 'file-item';
    div.setAttribute('tabindex', '0');
    div.dataset.index = String(index);
    if (f.url) div.title = f.url;
    var displayName = f.name;
    var subdir = '';
    var slashIdx = f.name.indexOf('/');
    if (slashIdx !== -1) {
      subdir = f.name.substring(0, slashIdx);
      displayName = f.name.substring(slashIdx + 1);
    }
    var fileExt = getFileTypeFromUrl(f.url);
    div.innerHTML = '<span class="file-badge ' + badgeClass + '">' + badgeText + '</span>' +
      fileTypeIconHtml(fileExt) +
      (subdir ? '<span class="file-subdir">' + subdir + '</span>' : '') +
      '<span class="file-name">' + displayName + '</span>';
    el.fileList.appendChild(div);
  }

  function renderFileList() {
    // Sort: new -> updated -> unchanged -> others
    var order = { 'new': 0, 'updated': 1, 'unchanged': 2, 'error': 3, 'info': 4 };
    var sorted = crawledFiles.slice().sort(function (a, b) {
      return (order[a.status] || 9) - (order[b.status] || 9);
    });

    var fragment = document.createDocumentFragment();
    for (var i = 0; i < sorted.length; i++) {
      var f = sorted[i];
      var badgeClass = f.status === 'new' ? 'new' : (f.status === 'updated' ? 'updated' : (f.status === 'unchanged' ? 'unchanged' : ''));
      var badgeText = f.status === 'new' ? '\u65b0\u589e' : (f.status === 'updated' ? '\u66f4\u65b0' : '\u672a\u53d8');
      var div = document.createElement('div');
      div.className = 'file-item';
      div.setAttribute('tabindex', '0');
      div.dataset.index = String(i);
      if (f.url) div.title = f.url;
      var displayName = f.name;
      var subdir = '';
      var slashIdx = f.name.indexOf('/');
      if (slashIdx !== -1) {
        subdir = f.name.substring(0, slashIdx);
        displayName = f.name.substring(slashIdx + 1);
      }
      var fileExt = getFileTypeFromUrl(f.url);
      div.innerHTML = '<span class="file-badge ' + badgeClass + '">' + badgeText + '</span>' +
        fileTypeIconHtml(fileExt) +
        (subdir ? '<span class="file-subdir">' + subdir + '</span>' : '') +
        '<span class="file-name">' + displayName + '</span>';
      fragment.appendChild(div);
    }
    el.fileList.innerHTML = '';
    el.fileList.appendChild(fragment);

    // Keyboard navigation on file list
    el.fileList.onkeydown = function (e) {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp' && e.key !== 'Enter') return;
      e.preventDefault();
      var items = el.fileList.querySelectorAll('.file-item[data-index]');
      if (!items.length) return;
      var current = el.fileList.querySelector('.file-item.selected');
      var idx = current ? parseInt(current.dataset.index, 10) : -1;
      if (e.key === 'ArrowDown') {
        idx = Math.min(idx + 1, items.length - 1);
      } else if (e.key === 'ArrowUp') {
        idx = Math.max(idx - 1, 0);
      } else if (e.key === 'Enter' && current) {
        current.click();
        return;
      }
      var target = items[idx];
      if (target) {
        var name = sorted[idx].name;
        var url = sorted[idx].url;
        selectFileItem(target, name, url);
        target.scrollIntoView({ block: 'nearest' });
      }
    };

    // Update counts
    var newC = 0, updC = 0, uncC = 0, errC = 0;
    for (var i = 0; i < crawledFiles.length; i++) {
      if (crawledFiles[i].status === 'new') newC++;
      else if (crawledFiles[i].status === 'updated') updC++;
      else if (crawledFiles[i].status === 'unchanged') uncC++;
      else if (crawledFiles[i].status === 'error') errC++;
    }
    el.newCount.textContent = newC;
    el.updatedCount.textContent = updC;
    el.unchangedCount.textContent = uncC;
    el.errorCount.textContent = errC;
    el.totalCount.textContent = crawledFiles.length;
  }

  function selectFileItem(element, name, url) {
    var prev = el.fileList.querySelector('.file-item.selected');
    if (prev) prev.classList.remove('selected');
    element.classList.add('selected');
    element.focus();
    loadPreview(name, url);
  }

  function getConfig() {
    var url = el.urls.value.trim();
    var urls = url ? [url] : [];
    var exts = [];
    el.extensions.querySelectorAll('input:checked').forEach(function (cb) { exts.push(cb.value); });
    return {
      crawler: {
        urls: urls,
        file_extensions: exts,
        content_format: el.contentFormat.value,
        meta_format: 'json',
        enable_meta: true,
        index_file: 'index.json',
        output_dir: el.outputDir.value,
        delay: parseFloat(el.delay.value) || 1,
        max_workers: 3,
        recursive: true,
        max_depth: parseInt(el.maxDepth.value) || 3,
        min_year: parseInt(el.minYear.value) || 2024
      }
    };
  }

  function toYAML(obj, indent) {
    indent = indent || 0;
    var spaces = '';
    for (var i = 0; i < indent; i++) spaces += '  ';
    var result = '';
    var keys = Object.keys(obj);
    for (var k = 0; k < keys.length; k++) {
      var key = keys[k];
      var val = obj[key];
      if (Array.isArray(val)) {
        result += spaces + key + ':\n';
        for (var j = 0; j < val.length; j++) {
          result += spaces + '  - ' + val[j] + '\n';
        }
      } else if (typeof val === 'object' && val !== null) {
        result += spaces + key + ':\n' + toYAML(val, indent + 1);
      } else {
        result += spaces + key + ': ' + val + '\n';
      }
    }
    return result;
  }

  function invoke(cmd, args) {
    if (window.__TAURI__ && window.__TAURI__.core) {
      return window.__TAURI__.core.invoke(cmd, args || {});
    }
    log('Tauri API 未就绪', 'error');
    return Promise.reject('Tauri API not available');
  }

  function listen(event, callback) {
    if (window.__TAURI__ && window.__TAURI__.event) {
      return window.__TAURI__.event.listen(event, callback);
    }
    return Promise.resolve(function () { });
  }

  function resetUI() {
    isRunning = false;
    el.startBtn.disabled = false;
    el.preCrawlBtn.disabled = false;
    el.stopBtn.disabled = true;
    el.statusDot.className = 'status-dot';
    lockConfigInputs(false);
  }

  function simpleMarkdown(text) {
    // Escape HTML but allow <br> tags
    var s = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    s = s.replace(/&lt;br&gt;/gi, '<br>');  // Restore <br> for table cell line breaks
    // Headers
    s = s.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>');
    s = s.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>');
    s = s.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>');
    s = s.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
    s = s.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
    s = s.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');
    // Bold and italic
    s = s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
    s = s.replace(/\*(.+?)\*/g, '<em>$1</em>');
    // Links
    s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" title="$2">$1</a>');
    // Images (disabled for RAG preview)
    // s = s.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img alt="$1" src="$2" style="max-width:100%">');
    // Horizontal rule
    s = s.replace(/^---+$/gm, '<hr>');
    // Tables: detect lines with | separators
    s = s.replace(/(^\|.+\|$\n?)+/gm, function (tableBlock) {
      var rows = tableBlock.trim().split('\n');
      var html = '<table>';
      for (var i = 0; i < rows.length; i++) {
        var row = rows[i].trim();
        if (!row) continue;
        // Skip separator row (|---|---|)
        if (/^\|[\s\-:|]+\|$/.test(row)) continue;
        var cells = row.split('|').filter(function (c, idx, arr) { return idx > 0 && idx < arr.length - 1; });
        var tag = (i === 0) ? 'th' : 'td';
        html += '<tr>';
        for (var j = 0; j < cells.length; j++) {
          html += '<' + tag + '>' + cells[j].trim() + '</' + tag + '>';
        }
        html += '</tr>';
      }
      html += '</table>';
      return html;
    });
    // List items
    s = s.replace(/^\s*[-*]\s+(.+)$/gm, '<li>$1</li>');
    // Paragraphs: double newline
    s = s.replace(/\n\n+/g, '</p><p>');
    s = '<p>' + s + '</p>';
    // Single newlines to <br>
    s = s.replace(/\n/g, '<br>');
    return s;
  }

  var currentPreviewMode = 'md';
  var currentPreviewFilename = '';

  function loadPreview(filename, url) {
    currentPreviewFilename = filename;
    el.previewTitle.textContent = filename;
    if (el.previewModeBtn) {
      el.previewModeBtn.style.display = 'inline-block';
      el.previewModeBtn.textContent = currentPreviewMode === 'md' ? '查看源材料 (Raw)' : '返回 Markdown';
    }

    if (currentPreviewMode === 'md') {
      if (el.rawIframe) el.rawIframe.style.display = 'none';
      if (el.docxContainer) el.docxContainer.style.display = 'none';
      if (el.xlsxContainer) el.xlsxContainer.style.display = 'none';
      el.previewContent.style.display = 'block';
      el.previewContent.textContent = '\u52a0\u8f7d\u4e2d...';
      var outputDir = el.outputDir.value || './output';
      invoke('read_file_content', { outputDir: outputDir, filename: filename }).then(function (html) {
        el.previewContent.innerHTML = html;
      }, function (e) {
        el.previewContent.textContent = '\u8bfb\u53d6\u5931\u8d25: ' + e;
      });
    } else {
      el.previewContent.style.display = 'none';
      if (el.rawIframe) {
        el.rawIframe.style.display = 'block';
        el.rawIframe.removeAttribute('srcdoc');
        var outputDir = el.outputDir.value || './output';
        invoke('get_raw_file_info', { outputDir: outputDir, filename: filename }).then(function (info) {
          if (info.is_text) {
            if (el.docxContainer) el.docxContainer.style.display = 'none';
            if (el.xlsxContainer) el.xlsxContainer.style.display = 'none';
            el.rawIframe.style.display = 'block';
            var htmlContent = info.content;
            if (info.source_url) {
              var baseTag = '<base href="' + info.source_url + '">';
              if (htmlContent.match(/<head[^>]*>/i)) {
                htmlContent = htmlContent.replace(/(<head[^>]*>)/i, '$1\n' + baseTag);
              } else {
                htmlContent = baseTag + '\n' + htmlContent;
              }
            }
            el.rawIframe.srcdoc = htmlContent;
          } else if (info.is_pdf && info.base64) {
            if (el.docxContainer) el.docxContainer.style.display = 'none';
            if (el.xlsxContainer) el.xlsxContainer.style.display = 'none';
            el.rawIframe.style.display = 'block';
            el.rawIframe.removeAttribute('srcdoc');
            el.rawIframe.src = 'data:application/pdf;base64,' + info.base64;
          } else if (info.is_docx && info.base64 && window.docx) {
            el.rawIframe.style.display = 'none';
            if (el.xlsxContainer) el.xlsxContainer.style.display = 'none';
            if (el.docxContainer) {
              el.docxContainer.style.display = 'block';
              el.docxContainer.innerHTML = '<div style="padding:20px;text-align:center;">\u6b63\u5728\u6e32\u67d3 DOCX \u6587\u6863...</div>';
              setTimeout(function () {
                var binaryString = atob(info.base64);
                var len = binaryString.length;
                var bytes = new Uint8Array(len);
                for (var i = 0; i < len; i++) {
                  bytes[i] = binaryString.charCodeAt(i);
                }
                var blob = new Blob([bytes], { type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document" });
                window.docx.renderAsync(blob, el.docxContainer).catch(function (e) {
                  el.docxContainer.innerHTML = '<div style="padding:20px;color:red;text-align:center;">DOCX\u6e32\u67d3\u5931\u8d25: ' + e + '</div>';
                });
              }, 50);
            }
          } else if (info.is_xlsx && info.base64 && window.XLSX) {
            el.rawIframe.style.display = 'none';
            if (el.docxContainer) el.docxContainer.style.display = 'none';
            if (el.xlsxContainer) {
              el.xlsxContainer.style.display = 'block';
              el.xlsxContainer.innerHTML = '<div style="padding:20px;text-align:center;">\u6b63\u5728\u89e3\u6790 Excel \u8868\u683c...</div>';
              setTimeout(function () {
                try {
                  var binaryString = atob(info.base64);
                  var wb = window.XLSX.read(binaryString, { type: 'binary' });
                  var ws = wb.Sheets[wb.SheetNames[0]];
                  var htmlstr = window.XLSX.utils.sheet_to_html(ws);
                  el.xlsxContainer.innerHTML = '<div style="font-family:sans-serif; margin-bottom:10px;"><b>\u5de5\u4f5c\u8868: ' + wb.SheetNames[0] + '</b></div>' +
                    '<style>#xlsxContainer table {border-collapse:collapse; font-size:13px;} #xlsxContainer td, #xlsxContainer th {border:1px solid #ccc; padding:4px 8px;}</style>' + htmlstr;
                } catch (e) {
                  el.xlsxContainer.innerHTML = '<div style="padding:20px;color:red;text-align:center;">Excel \u89e3\u6790\u5931\u8d25: ' + e + '</div>';
                }
              }, 50);
            }
          } else {
            if (el.docxContainer) el.docxContainer.style.display = 'none';
            if (el.xlsxContainer) el.xlsxContainer.style.display = 'none';
            el.rawIframe.style.display = 'block';
            var safePath = info.path.replace(/\\/g, '\\\\');
            var html = '<div style="font-family:sans-serif; text-align:center; padding: 50px;">' +
              '<h3>\u8be5\u6587\u4ef6\u7c7b\u578b (.' + info.ext + ') \u65e0\u6cd5\u76f4\u63a5\u5185\u5d4c\u9884\u89c8</h3>' +
              '<p style="color:#666; margin-bottom:20px; font-size:12px; word-break:break-all;">' + info.path + '</p>' +
              '<button onclick="window.parent.__TAURI__.core.invoke(\'open_url\', {url:\'' + safePath + '\'})" ' +
              'style="padding:8px 16px; cursor:pointer; background:#3b82f6; color:#fff; border:none; border-radius:4px;">' +
              '\u8c03\u7528\u7cfb\u7edf\u9ed8\u8ba4\u5e94\u7528\u6253\u5f00</button></div>';
            el.rawIframe.srcdoc = html;
          }
        }, function (e) {
          el.rawIframe.srcdoc = '<div style="font-family:sans-serif; padding:20px; color:red;">\u627e\u4e0d\u5230\u6e90\u6587\u4ef6\u5bf9\u5e94\u7684\u5907\u4efd\uff1a' + e + '</div>';
        });
      }
    }
  }

  if (el.previewModeBtn) {
    el.previewModeBtn.addEventListener('click', function () {
      currentPreviewMode = currentPreviewMode === 'md' ? 'raw' : 'md';
      if (currentPreviewFilename) loadPreview(currentPreviewFilename, '');
    });
  }

  // Auto-save config on any change (debounced)
  var saveTimer = null;
  function autoSaveConfig() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(function () {
      var yaml = toYAML(getConfig());
      invoke('write_config', { configPath: configPath, content: yaml });
    }, 500);
  }

  // Bind auto-save to all config inputs (except delay which has special handling)
  var configInputs = [el.urls, el.outputDir, el.contentFormat, el.maxDepth, el.minYear];
  for (var ci = 0; ci < configInputs.length; ci++) {
    configInputs[ci].addEventListener('input', autoSaveConfig);
    configInputs[ci].addEventListener('change', autoSaveConfig);
  }
  el.extensions.addEventListener('change', autoSaveConfig);

  // Delay: auto-save + real-time update via Tauri event
  el.delay.addEventListener('change', function () {
    autoSaveConfig();
    // Send delay update to running crawler if active
    if (isRunning && window.__TAURI__ && window.__TAURI__.core) {
      invoke('update_delay', { delay: parseFloat(el.delay.value) || 0.5 });
    }
  });

  // Lock/unlock all UI during crawl (only stop button remains active)
  function lockConfigInputs(lock) {
    var inputs = [el.urls, el.outputDir, el.contentFormat, el.maxDepth, el.minYear];
    for (var i = 0; i < inputs.length; i++) {
      inputs[i].disabled = lock;
    }
    el.extensions.querySelectorAll('input').forEach(function (cb) { cb.disabled = lock; });
    // Lock action buttons
    var clearBtn = document.getElementById('clearBtn');
    if (clearBtn) clearBtn.disabled = lock;
    if (el.browseDirBtn) el.browseDirBtn.disabled = lock;
    if (el.openDirBtn) el.openDirBtn.disabled = lock;
    // Lock site-item action buttons (delete) but allow opening folders & site list
    document.querySelectorAll('.site-delete').forEach(function (btn) {
      btn.style.pointerEvents = lock ? 'none' : '';
      btn.style.opacity = lock ? '0.3' : '';
    });
    // delay is always editable
  }

  function onCrawlComplete(result) {
    log(result.message, result.success ? 'success' : 'error');

    var msg = result.message || '';
    var totalMatch = msg.match(/Total:\s*(\d+)/);
    var newMatch = msg.match(/New:\s*(\d+)/);
    var updMatch = msg.match(/Updated:\s*(\d+)/);
    var uncMatch = msg.match(/Unchanged:\s*(\d+)/);
    var errMatch = msg.match(/Errors:\s*(\d+)/);
    if (totalMatch) el.totalCount.textContent = totalMatch[1];
    if (newMatch) el.newCount.textContent = newMatch[1];
    if (updMatch) el.updatedCount.textContent = updMatch[1];
    if (uncMatch) el.unchangedCount.textContent = uncMatch[1];
    if (errMatch) el.errorCount.textContent = errMatch[1];

    // Load file list from index.json, filtered by crawled site
    var outputDir = el.outputDir.value || './output';
    var crawlSite = getActiveSiteFromUrl();
    // Refresh site list and auto-select
    loadSiteList(crawlSite);
    invoke('read_index', { outputDir: outputDir }).then(function (indexStr) {
      try {
        var indexData = JSON.parse(indexStr);
        var fileTree = indexData.file_tree || {};
        var newSet = {};
        var updSet = {};
        if (result.new_files) { for (var i = 0; i < result.new_files.length; i++) newSet[result.new_files[i]] = true; }
        if (result.updated_files) { for (var i = 0; i < result.updated_files.length; i++) updSet[result.updated_files[i]] = true; }
        crawledFiles = [];
        el.fileList.innerHTML = '';
        var names = Object.keys(fileTree);
        for (var i = 0; i < names.length; i++) {
          var name = names[i];
          // Filter by crawled site if known
          if (crawlSite && name.indexOf(crawlSite + '/') !== 0) continue;
          var meta = fileTree[name];
          var st = newSet[name] ? 'new' : (updSet[name] ? 'updated' : 'unchanged');
          addFileToList(name, st, meta.source_url || '');
        }
      } catch (e) {
        log('\u8bfb\u53d6\u7d22\u5f15\u5931\u8d25: ' + e, 'error');
      }
    });

    el.progressFill.style.width = '100%';
    var t = totalMatch ? totalMatch[1] : '0';
    var n = newMatch ? newMatch[1] : '0';
    var u = updMatch ? updMatch[1] : '0';
    var uc = uncMatch ? uncMatch[1] : '0';
    el.progressText.textContent = '\u5b8c\u6210 - \u603b\u8ba1 ' + t + ', \u65b0\u589e ' + n + ', \u66f4\u65b0 ' + u + ', \u672a\u53d8 ' + uc;
    el.statusText.textContent = '\u5b8c\u6210';
    resetUI();
  }

  // --- Site list management ---
  function loadSiteList(autoSelectSite) {
    var outputDir = el.outputDir.value || './output';
    invoke('list_crawled_sites', { outputDir: outputDir }).then(function (jsonStr) {
      try {
        var sites = JSON.parse(jsonStr);
        el.siteList.innerHTML = '';
        if (sites.length === 0) {
          el.siteList.innerHTML = '<div class="site-item empty">暂无站点</div>';
          return;
        }
        for (var i = 0; i < sites.length; i++) {
          (function (site) {
            var div = document.createElement('div');
            div.className = 'site-item';
            div.innerHTML = '<span class="site-name">' + site.name + '</span>' +
              '<span class="site-count">' + site.file_count + '</span>' +
              '<span class="site-open" title="打开目录">📂</span>' +
              '<span class="site-delete" title="删除站点">🗑️</span>';
            div.querySelector('.site-open').addEventListener('click', function (e) {
              e.stopPropagation();
              var base = el.outputDir.value || './output';
              invoke('open_url', { url: base + '/' + site.name });
            });
            div.querySelector('.site-delete').addEventListener('click', function (e) {
              e.stopPropagation();
              if (confirm('确定要彻底删除站点目录 "' + site.name + '" 吗？此操作不可恢复。')) {
                var base = el.outputDir.value || './output';
                invoke('delete_site', { outputDir: base, siteName: site.name }).then(function () {
                  if (activeSite === site.name) {
                    activeSite = null;
                    el.fileList.innerHTML = '<div class="file-item"><span>暂无文件</span></div>';
                    el.statusText.textContent = '站点已删除';
                  }
                  loadSiteList();
                }).catch(function (err) {
                  alert('删除失败: ' + err);
                });
              }
            });
            div.addEventListener('click', function () {
              if (isRunning) return;
              selectSite(site.name, div);
            });
            el.siteList.appendChild(div);
            if ((autoSelectSite === true && i === 0) || (autoSelectSite && site.name === autoSelectSite)) {
              selectSite(site.name, div);
            }
          })(sites[i]);
        }
      } catch (e) { }
    }, function () { });
  }

  function selectSite(siteName, element) {
    activeSite = siteName;
    var items = el.siteList.querySelectorAll('.site-item');
    for (var i = 0; i < items.length; i++) items[i].classList.remove('active');
    if (element) element.classList.add('active');
    loadSiteFiles(siteName);
    // Load and fill site config into the settings panel
    if (!isRunning) {
      var outputDir = el.outputDir.value || './output';
      invoke('read_site_config', { outputDir: outputDir, siteName: siteName }).then(function (jsonStr) {
        try {
          var cfg = JSON.parse(jsonStr);
          if (!cfg.url) return;
          // Fill target URL
          el.urls.value = cfg.url;
          // Fill max depth
          if (cfg.max_depth !== undefined) el.maxDepth.value = cfg.max_depth;
          // Fill delay
          if (cfg.delay !== undefined) el.delay.value = cfg.delay;
          // Fill min year
          if (cfg.min_year !== undefined) el.minYear.value = cfg.min_year;
          // Fill content format
          if (cfg.content_format) el.contentFormat.value = cfg.content_format;
          // Fill file extensions checkboxes
          if (cfg.file_extensions && cfg.file_extensions.length > 0) {
            var checkboxes = el.extensions.querySelectorAll('input[type="checkbox"]');
            for (var i = 0; i < checkboxes.length; i++) {
              checkboxes[i].checked = cfg.file_extensions.indexOf(checkboxes[i].value) >= 0;
            }
          }
          log('已加载站点配置: ' + siteName, 'info');
        } catch (e) { }
      }, function () { });
    }
  }

  function loadSiteFiles(siteName) {
    var outputDir = el.outputDir.value || './output';
    invoke('read_site_index', { outputDir: outputDir, siteName: siteName }).then(function (indexStr) {
      try {
        var indexData = JSON.parse(indexStr);
        var fileTree = indexData.file_tree || {};
        crawledFiles = [];
        var names = Object.keys(fileTree);
        for (var i = 0; i < names.length; i++) {
          var name = names[i];
          var meta = fileTree[name];
          crawledFiles.push({ name: name, status: 'unchanged', url: meta.source_url || '' });
        }
        renderFileList();
        el.totalCount.textContent = names.length;
        el.unchangedCount.textContent = names.length;
        el.newCount.textContent = '0';
        el.updatedCount.textContent = '0';
        el.errorCount.textContent = '0';
        el.progressFill.style.width = names.length > 0 ? '100%' : '0%';
        el.progressText.textContent = siteName + ': ' + names.length + ' 个文件';
      } catch (e) { }
    }, function () { });
  }

  function getActiveSiteFromUrl() {
    var url = el.urls.value.trim();
    if (!url) return null;
    try {
      // Handle file:// URLs: convert path to subdir name
      if (url.indexOf('file://') === 0) {
        var path = url.substring(7); // strip file://
        return path.replace(/^\/+|\/+$/g, '').replace(/[\/\\]/g, '_').replace(/[^\w\-]/g, '_') || null;
      }
      var a = document.createElement('a');
      a.href = url;
      return a.hostname || null;
    } catch (e) { return null; }
  }

  // Browse directory button
  if (el.browseDirBtn) {
    el.browseDirBtn.addEventListener('click', function () {
      if (typeof window.__TAURI__ !== 'undefined' && window.__TAURI__.dialog) {
        window.__TAURI__.dialog.open({ directory: true, title: '选择输出目录' }).then(function (path) {
          if (path) {
            el.outputDir.value = path;
            el.outputDir.dispatchEvent(new Event('change'));
          }
        }).catch(function (err) { alert('Error: ' + err); });
      }
    });
  }

  // Pre-crawl: discover all URLs without downloading
  el.preCrawlBtn.addEventListener('click', function () {
    if (isRunning) return;
    isRunning = true;
    var myGen = ++crawlGeneration;
    el.preCrawlBtn.disabled = true;
    el.startBtn.disabled = true;
    el.stopBtn.disabled = false;
    lockConfigInputs(true);
    el.statusDot.className = 'status-dot active';
    el.statusText.textContent = '预爬中...';
    el.logContainer.innerHTML = '';
    el.progressFill.style.width = '0%';
    el.progressText.textContent = '正在发现URL...';
    log('全站预爬开始...');

    var unlistenPre = null;
    var unlistenLog = null;
    listen('pre-crawl-progress', function (event) {
      var d = event.payload;
      var docTag = d.is_doc ? ' [文档]' : '';
      el.progressText.textContent = '已发现 ' + d.found + ' 个 URL，当前深度 ' + d.depth;
    }).then(function (fn) { unlistenPre = fn; });
    listen('crawl-progress', function (event) {
      var data = event.payload;
      if (data.line) log(data.line);
    }).then(function (fn) { unlistenLog = fn; });

    function cleanupListeners() {
      if (unlistenPre) unlistenPre();
      if (unlistenLog) unlistenLog();
    }

    var yaml = toYAML(getConfig());
    invoke('write_config', { configPath: configPath, content: yaml }).then(function () {
      return invoke('run_pre_crawl', { configPath: configPath });
    }).then(function (jsonStr) {
      preCrawlData = JSON.parse(jsonStr);
      invoke('save_pre_crawl_result', { configPath: configPath, data: jsonStr });
      log('预爬完成: 发现 ' + preCrawlData.total + ' 个 URL，最大深度 ' + preCrawlData.max_depth, 'success');
      var depths = preCrawlData.urls_per_depth || {};
      var by = preCrawlData.by_depth || {};
      for (var d in depths) {
        log('  深度 ' + d + ': ' + depths[d] + ' 个 URL (累计: ' + (by[d] || 0) + ')', 'info');
      }
      el.progressFill.style.width = '100%';
      el.progressText.textContent = '预爬完成: ' + preCrawlData.total + ' 个 URL, 最大深度 ' + preCrawlData.max_depth;
      el.statusText.textContent = '预爬完成';
      cleanupListeners();
      if (myGen === crawlGeneration) resetUI();
    }, function (e) {
      log('预爬失败: ' + e, 'error');
      el.statusText.textContent = '预爬失败';
      cleanupListeners();
      if (myGen === crawlGeneration) resetUI();
    });
  });

  // Start crawl
  el.startBtn.addEventListener('click', function () {
    if (isRunning) return;

    isRunning = true;
    var myGen = ++crawlGeneration;
    crawledFiles = [];
    var crawlStartTime = Date.now();

    // Estimate total based on configured max depth
    var estimatedTotal = 0;
    if (preCrawlData) {
      var maxDepthSetting = parseInt(el.maxDepth.value, 10);
      if (maxDepthSetting >= 999) {
        estimatedTotal = preCrawlData.total;
      } else {
        var byDepth = preCrawlData.by_depth || {};
        estimatedTotal = byDepth[String(maxDepthSetting - 1)] || preCrawlData.total;
      }
    }
    el.startBtn.disabled = true;
    el.preCrawlBtn.disabled = true;
    el.stopBtn.disabled = false;
    lockConfigInputs(true);
    el.statusDot.className = 'status-dot active';
    el.statusText.textContent = '爬取中...';
    el.logContainer.innerHTML = '';
    el.fileList.innerHTML = '';
    el.newCount.textContent = '0';
    el.updatedCount.textContent = '0';
    el.unchangedCount.textContent = '0';
    el.errorCount.textContent = '0';
    el.totalCount.textContent = '0';
    el.progressFill.style.width = '0%';
    if (estimatedTotal > 0) {
      el.progressText.textContent = '0 / ' + estimatedTotal;
    }

    // Register event listener for real-time progress
    var unlisten = null;
    var processedCount = 0;
    listen('crawl-progress', function (event) {
      var data = event.payload;
      var logType = 'info';
      if (data.status === 'new') logType = 'success';
      else if (data.status === 'updated') logType = 'success';
      else if (data.status === 'error') logType = 'error';
      else if (data.line && data.line.indexOf('[skip]') !== -1) logType = 'skip';
      log(data.line, logType);
      if (data.file_name && data.status !== 'info') {
        addFileToList(data.file_name, data.status, data.url || '');
      }
      // Count processed pages from Crawling: lines
      if (data.line.indexOf('Crawling:') !== -1) {
        processedCount++;
        var crawlUrl = data.line.split('Crawling:')[1];
        if (crawlUrl) el.statusText.textContent = '正在爬取: ' + crawlUrl.trim();
        // Progress with estimation
        if (estimatedTotal > 0) {
          var pct = Math.min(99, Math.round(processedCount / estimatedTotal * 100));
          el.progressFill.style.width = pct + '%';
          // Time estimation
          var elapsed = (Date.now() - crawlStartTime) / 1000;
          var rate = processedCount / elapsed; // pages per second
          var remaining = rate > 0 ? Math.round((estimatedTotal - processedCount) / rate) : 0;
          var etaStr = remaining > 60 ? Math.round(remaining / 60) + '分' + (remaining % 60) + '秒' : remaining + '秒';
          el.progressText.textContent = processedCount + ' / ' + estimatedTotal + ' (预计剩余 ' + etaStr + ')';
        } else {
          el.progressText.textContent = '已处理 ' + processedCount + ' 个页面';
        }
      }
    }).then(function (fn) { unlisten = fn; });

    // Start crawl flow - config is already auto-saved
    log('开始爬取...');
    if (estimatedTotal > 0) {
      log('预爬数据: 预计 ' + estimatedTotal + ' 个 URL', 'info');
    }
    var yaml = toYAML(getConfig());

    invoke('write_config', { configPath: configPath, content: yaml }).then(function () {
      var cfg = getConfig();
      for (var u = 0; u < cfg.crawler.urls.length; u++) {
        log('目标URL: ' + cfg.crawler.urls[u], 'info');
      }
      return invoke('run_crawler', { configPath: configPath });
    }).then(function (result) {
      if (myGen === crawlGeneration) onCrawlComplete(result);
      if (unlisten) unlisten();
    }, function (e) {
      log('爬取失败: ' + e, 'error');
      el.statusText.textContent = '已停止';
      if (myGen === crawlGeneration) {
        resetUI();
        var crawlSite = getActiveSiteFromUrl();
        loadSiteList(crawlSite);
      }
      if (unlisten) unlisten();
    });
  });

  // Stop - only send signal; actual UI reset is handled by crawl completion callback
  el.stopBtn.addEventListener('click', function () {
    el.stopBtn.disabled = true;
    el.statusText.textContent = '正在停止...';
    log('正在停止...');
    invoke('stop_crawler').then(function (msg) {
      log('已停止: ' + msg, 'info');
    }, function (e) {
      log('停止失败: ' + e, 'error');
    });
  });

  // Clear crawl results
  document.getElementById('clearBtn').addEventListener('click', function () {
    if (isRunning) {
      log('\u722c\u53d6\u8fdb\u884c\u4e2d\uff0c\u65e0\u6cd5\u6e05\u7a7a', 'error');
      return;
    }
    // Determine which site(s) to clear
    var subdirs = [];
    if (activeSite) {
      subdirs.push(activeSite);
    } else {
      var url = el.urls.value.trim();
      if (url) {
        try {
          if (url.indexOf('://') === -1) url = 'https://' + url;
          var a = document.createElement('a');
          a.href = url;
          if (a.hostname) subdirs.push(a.hostname);
        } catch (e) { }
      }
    }
    if (subdirs.length === 0) {
      log('\u6ca1\u6709\u9009\u4e2d\u7ad9\u70b9\u6216\u914d\u7f6e\u76ee\u6807URL', 'error');
      return;
    }
    var outputDir = el.outputDir.value || './output';
    log('\u6b63\u5728\u6e05\u7a7a: ' + subdirs.join(', ') + ' ...', 'info');
    invoke('clear_output', { outputDir: outputDir, subdirs: subdirs }).then(function (msg) {
      log(msg, 'success');
      // Reset UI
      crawledFiles = [];
      el.fileList.innerHTML = '<div class="file-item"><span>\u6682\u65e0\u6587\u4ef6</span></div>';
      el.newCount.textContent = '0';
      el.updatedCount.textContent = '0';
      el.unchangedCount.textContent = '0';
      el.errorCount.textContent = '0';
      el.totalCount.textContent = '0';
      el.progressFill.style.width = '0%';
      el.progressText.textContent = '\u51c6\u5907\u5c31\u7eea';
      el.previewContent.textContent = '\u5355\u51fb\u5de6\u4fa7\u6587\u4ef6\u5373\u53ef\u9884\u89c8\u5185\u5bb9';
      el.previewTitle.textContent = '\u9009\u62e9\u6587\u4ef6\u9884\u89c8';
      // Refresh site list to remove cleared sites
      activeSite = null;
      loadSiteList();
    }, function (e) {
      log('\u6e05\u7a7a\u5931\u8d25: ' + e, 'error');
    });
  });

  // Init - populate minYear dropdown
  (function initMinYear() {
    var currentYear = new Date().getFullYear();
    var lastYear = currentYear - 1;
    for (var y = currentYear; y >= currentYear - 5; y--) {
      var opt = document.createElement('option');
      opt.value = String(y);
      opt.textContent = y + ' 年';
      if (y === lastYear) opt.selected = true;
      el.minYear.appendChild(opt);
    }
  })();

  // Init
  var recentDirs = [];
  try {
    recentDirs = JSON.parse(localStorage.getItem('dp-recent-dirs')) || [];
  } catch (e) { }
  function renderRecentDirs() {
    var dl = document.getElementById('recentOutputDirs');
    if (!dl) return;
    dl.innerHTML = '';
    for (var i = 0; i < recentDirs.length; i++) {
      var opt = document.createElement('option');
      opt.value = recentDirs[i];
      dl.appendChild(opt);
    }
  }
  function updateRecentDirsList(newPath) {
    if (!newPath) return;
    invoke('get_absolute_path', { path: newPath }).then(function (absPath) {
      if (el.outputDir.value === newPath) el.outputDir.value = absPath;
      var idx = recentDirs.indexOf(absPath);
      if (idx !== -1) recentDirs.splice(idx, 1);
      recentDirs.unshift(absPath);
      if (recentDirs.length > 10) recentDirs.length = 10;
      localStorage.setItem('dp-recent-dirs', JSON.stringify(recentDirs));
      renderRecentDirs();
    }).catch(function (e) { log('\u8def\u5f84\u89e3\u6790\u5931\u8d25: ' + e, 'error'); });
  }
  renderRecentDirs();

  el.outputDir.addEventListener('change', function () {
    if (isRunning) return;
    updateRecentDirsList(el.outputDir.value);
    loadSiteList(true);
    el.fileList.innerHTML = '<div class="file-item"><span>\u6682\u65e0\u6587\u4ef6</span></div>';
    el.totalCount.textContent = '0';
    el.unchangedCount.textContent = '0';
    el.newCount.textContent = '0';
    el.updatedCount.textContent = '0';
    el.errorCount.textContent = '0';
    el.progressFill.style.width = '0%';
    el.progressText.textContent = '\u76ee\u5f55\u5df2\u5207\u6362';
    autoSaveConfig();
  });

  function loadSavedConfig() {
    invoke('read_config', { configPath: configPath }).then(function (yamlStr) {
      // Simple YAML parser for our config structure
      var lines = yamlStr.split('\n');
      var cfg = {};
      for (var i = 0; i < lines.length; i++) {
        var line = lines[i].trim();
        if (!line || line.indexOf('#') === 0) continue;
        // Parse list items
        if (line.indexOf('- ') === 0) {
          var val = line.substring(2).trim();
          if (lastKey === 'urls') urls.push(val);
          else if (lastKey === 'file_extensions') exts.push(val);
          continue;
        }
        var colon = line.indexOf(':');
        if (colon === -1) continue;
        var key = line.substring(0, colon).trim();
        var value = line.substring(colon + 1).trim();
        cfg[key] = value;
        if (key === 'urls') { var urls = []; var lastKey = 'urls'; }
        else if (key === 'file_extensions') { var exts = []; var lastKey = 'file_extensions'; }
        else { lastKey = key; }
      }
      // Fill form
      if (urls && urls.length) el.urls.value = urls.join('\n');
      if (cfg.output_dir) el.outputDir.value = cfg.output_dir;
      if (cfg.content_format) el.contentFormat.value = cfg.content_format;
      if (cfg.delay) {
        var options = el.delay.options;
        for (var i = 0; i < options.length; i++) {
          if (options[i].value === cfg.delay) { el.delay.value = cfg.delay; break; }
        }
      }
      if (cfg.max_depth) el.maxDepth.value = cfg.max_depth;
      if (cfg.min_year) el.minYear.value = cfg.min_year;
      // recursive is now hardcoded to true
      if (exts && exts.length) {
        el.extensions.querySelectorAll('input').forEach(function (cb) {
          cb.checked = exts.indexOf(cb.value) !== -1;
        });
      }
      updateRecentDirsList(el.outputDir.value);
      var targetSite = null;
      var curUrl = el.urls.value.trim().split('\n')[0];
      if (curUrl) {
        try {
          if (curUrl.indexOf('://') === -1) curUrl = 'https://' + curUrl;
          var a = document.createElement('a');
          a.href = curUrl;
          if (a.hostname) targetSite = a.hostname;
        } catch (e) { }
      }
      loadSiteList(targetSite || true);
      log('\u914d\u7f6e\u5df2\u52a0\u8f7d', 'success');
    }, function () {
      updateRecentDirsList(el.outputDir.value);
      log('\u672a\u627e\u5230\u914d\u7f6e\u6587\u4ef6\uff0c\u4f7f\u7528\u9ed8\u8ba4\u914d\u7f6e', 'info');
    });
  }

  function loadLastResults() {
    var outputDir = el.outputDir.value || './output';
    invoke('read_index', { outputDir: outputDir }).then(function (indexStr) {
      try {
        var indexData = JSON.parse(indexStr);
        var fileTree = indexData.file_tree || {};
        var names = Object.keys(fileTree);
        if (names.length === 0) return;
        crawledFiles = [];
        for (var i = 0; i < names.length; i++) {
          var name = names[i];
          var meta = fileTree[name];
          crawledFiles.push({ name: name, status: 'unchanged', url: meta.source_url || '' });
        }
        renderFileList();
        el.totalCount.textContent = names.length;
        el.unchangedCount.textContent = names.length;
        el.progressFill.style.width = '100%';
        el.progressText.textContent = '\u4e0a\u6b21\u722c\u53d6: ' + names.length + ' \u4e2a\u6587\u4ef6';
        log('\u5df2\u52a0\u8f7d\u4e0a\u6b21\u722c\u53d6\u7ed3\u679c: ' + names.length + ' \u4e2a\u6587\u4ef6', 'success');
      } catch (e) { /* no previous results */ }
    }, function () { /* no index file yet */ });
  }

  if (window.__TAURI__ && window.__TAURI__.core) {
    log('DPCrawler \u5df2\u5c31\u7eea', 'success');
    loadSavedConfig();
    loadSiteList();
    loadLastResults();
    // Load pre-crawl data from disk if available
    invoke('load_pre_crawl_result', { configPath: configPath }).then(function (jsonStr) {
      try {
        preCrawlData = JSON.parse(jsonStr);
        log('\u5df2\u52a0\u8f7d\u9884\u722c\u6570\u636e: ' + preCrawlData.total + ' \u4e2a URL', 'info');
      } catch (e) { }
    }, function () { });
  } else {
    log('Tauri API \u672a\u68c0\u6d4b\u5230', 'error');
  }

  // About dialog
  document.getElementById('aboutBtn').addEventListener('click', async function () {
    // 获取版本信息
    let versionInfo = { version: '1.0.0', full_version: '1.0.0', git_hash: 'unknown', git_date: 'unknown' };
    try {
      versionInfo = await window.__TAURI__.core.invoke('get_app_version');
    } catch (e) {
      console.error('Failed to get app version:', e);
    }

    var overlay = document.createElement('div');
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.4);z-index:9999;display:flex;align-items:center;justify-content:center';
    overlay.innerHTML = '<div style="background:#fff;border-radius:12px;padding:32px 40px;text-align:center;box-shadow:0 8px 32px rgba(0,0,0,0.15);max-width:360px">' +
      '<img src="favicon.png" style="width:64px;height:64px;margin-bottom:12px">' +
      '<h2 style="margin:0 0 4px;font-size:20px;color:#1e293b">DPCrawler</h2>' +
      '<p style="margin:0 0 8px;font-size:13px;color:#64748b">RAG\u77e5\u8bc6\u722c\u866b</p>' +
      '<p style="margin:0 0 4px;font-size:12px;color:#94a3b8">v' + versionInfo.full_version + '</p>' +
      '<p style="margin:0 0 4px;font-size:11px;color:#b0b8c4;font-family:monospace">commit: ' + versionInfo.git_hash + '</p>' +
      '<p style="margin:0 0 16px;font-size:11px;color:#b0b8c4">' + versionInfo.git_date + '</p>' +
      '<p style="margin:0 0 16px;font-size:12px;color:#94a3b8">\u00a9 2026 DEEPAI GROUP</p>' +
      '<button style="padding:6px 24px;border:none;background:#3b82f6;color:#fff;border-radius:6px;cursor:pointer;font-size:13px" onclick="this.closest(\'div[style]\').parentElement.remove()">\u786e\u5b9a</button>' +
      '</div>';
    overlay.addEventListener('click', function (e) { if (e.target === overlay) overlay.remove(); });
    document.body.appendChild(overlay);
  });

  // Window starts hidden (visible:false in tauri.conf.json)
  // Show after DOM is fully mounted and ready
  var appWindow = window.__TAURI__.window.getCurrentWindow();
  appWindow.show();
}); // end DOMContentLoaded
