// DPCrawler Frontend
document.addEventListener('DOMContentLoaded', function() {

var configPath = 'config.yaml';
var isRunning = false;
var crawledFiles = [];

var el = {
  urls: document.getElementById('urls'),
  extensions: document.getElementById('extensions'),
  outputDir: document.getElementById('outputDir'),
  contentFormat: document.getElementById('contentFormat'),
  delay: document.getElementById('delay'),
  maxDepth: document.getElementById('maxDepth'),
  recursive: document.getElementById('recursive'),
  startBtn: document.getElementById('startBtn'),
  stopBtn: document.getElementById('stopBtn'),
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
  previewOpenBtn: document.getElementById('previewOpenBtn')
};

// Tab switching
document.querySelectorAll('.tab').forEach(function(tab) {
  tab.addEventListener('click', function() {
    document.querySelectorAll('.tab').forEach(function(t) { t.classList.remove('active'); });
    document.querySelectorAll('.tab-content').forEach(function(c) { c.classList.remove('active'); });
    tab.classList.add('active');
    document.getElementById(tab.dataset.tab).classList.add('active');
  });
});

// Panel resizer drag
(function() {
  var resizer = document.getElementById('panelResizer');
  var fileList = document.getElementById('fileList');
  if (!resizer || !fileList) return;
  var startX, startW;
  resizer.addEventListener('mousedown', function(e) {
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

function log(msg, type) {
  type = type || 'info';
  var span = document.createElement('span');
  span.className = 'log-' + type;
  span.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg + '\n';
  el.logContainer.appendChild(span);
  el.logContainer.scrollTop = el.logContainer.scrollHeight;
}

// File type icon using file-icon-vectors (classic style)
function getFileTypeFromUrl(url) {
  if (!url) return 'html';
  try {
    var pathname = new URL(url).pathname;
    var dot = pathname.lastIndexOf('.');
    if (dot !== -1) {
      var ext = pathname.substring(dot + 1).toLowerCase();
      if (ext && ext.length <= 4) return ext;
    }
  } catch(e) {}
  return 'html';
}

function fileTypeIconHtml(ext) {
  return '<span class="file-type-icon"><span class="fiv-cla fiv-icon-' + ext + '"></span></span>';
}

function addFileToList(name, status, url) {
  if (!name) return;
  crawledFiles.push({ name: name, status: status, url: url || '' });
  renderFileList();
}

function renderFileList() {
  // Sort: new -> updated -> unchanged -> others
  var order = { 'new': 0, 'updated': 1, 'unchanged': 2, 'error': 3, 'info': 4 };
  var sorted = crawledFiles.slice().sort(function(a, b) {
    return (order[a.status] || 9) - (order[b.status] || 9);
  });

  el.fileList.innerHTML = '';
  for (var i = 0; i < sorted.length; i++) {
    var f = sorted[i];
    var badgeClass = f.status === 'new' ? 'new' : (f.status === 'updated' ? 'updated' : (f.status === 'unchanged' ? 'unchanged' : ''));
    var badgeText = f.status === 'new' ? '\u65b0\u589e' : (f.status === 'updated' ? '\u66f4\u65b0' : '\u672a\u53d8');
    var div = document.createElement('div');
    div.className = 'file-item';
    div.setAttribute('tabindex', '0');
    div.dataset.index = String(i);
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
    (function(name, url, element) {
      element.addEventListener('click', function() {
        selectFileItem(element, name, url);
      });
      if (url) {
        element.title = url;
        element.addEventListener('dblclick', function() {
          invoke('open_url', { url: url });
        });
      }
    })(f.name, f.url, div);
    el.fileList.appendChild(div);
  }

  // Keyboard navigation on file list
  el.fileList.onkeydown = function(e) {
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
  var items = el.fileList.querySelectorAll('.file-item');
  for (var j = 0; j < items.length; j++) items[j].classList.remove('selected');
  element.classList.add('selected');
  element.focus();
  loadPreview(name, url);
}

function getConfig() {
  var urls = el.urls.value.split('\n').map(function(u) { return u.trim(); }).filter(function(u) { return u; });
  var exts = [];
  el.extensions.querySelectorAll('input:checked').forEach(function(cb) { exts.push(cb.value); });
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
      recursive: el.recursive.checked,
      max_depth: parseInt(el.maxDepth.value) || 3
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
  return Promise.resolve(function() {});
}

function resetUI() {
  isRunning = false;
  el.startBtn.disabled = false;
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
  s = s.replace(/(^\|.+\|$\n?)+/gm, function(tableBlock) {
    var rows = tableBlock.trim().split('\n');
    var html = '<table>';
    for (var i = 0; i < rows.length; i++) {
      var row = rows[i].trim();
      if (!row) continue;
      // Skip separator row (|---|---|)
      if (/^\|[\s\-:|]+\|$/.test(row)) continue;
      var cells = row.split('|').filter(function(c, idx, arr) { return idx > 0 && idx < arr.length - 1; });
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

function loadPreview(filename, url) {
  el.previewTitle.textContent = filename;
  el.previewContent.textContent = '\u52a0\u8f7d\u4e2d...';
  if (url) {
    el.previewOpenBtn.style.display = '';
    el.previewOpenBtn.onclick = function() {
      invoke('open_url', { url: url });
    };
  } else {
    el.previewOpenBtn.style.display = 'none';
  }
  var outputDir = el.outputDir.value || './output';
  invoke('read_file_content', { outputDir: outputDir, filename: filename }).then(function(content) {
    el.previewContent.innerHTML = simpleMarkdown(content);
  }, function(e) {
    el.previewContent.textContent = '\u8bfb\u53d6\u5931\u8d25: ' + e;
  });
}

// Auto-save config on any change (debounced)
var saveTimer = null;
function autoSaveConfig() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(function() {
    var yaml = toYAML(getConfig());
    invoke('write_config', { configPath: configPath, content: yaml });
  }, 500);
}

// Bind auto-save to all config inputs (except delay which has special handling)
var configInputs = [el.urls, el.outputDir, el.contentFormat, el.maxDepth, el.recursive];
for (var ci = 0; ci < configInputs.length; ci++) {
  configInputs[ci].addEventListener('input', autoSaveConfig);
  configInputs[ci].addEventListener('change', autoSaveConfig);
}
el.extensions.addEventListener('change', autoSaveConfig);

// Delay: auto-save + real-time update via Tauri event
el.delay.addEventListener('change', function() {
  autoSaveConfig();
  // Send delay update to running crawler if active
  if (isRunning && window.__TAURI__ && window.__TAURI__.core) {
    invoke('update_delay', { delay: parseFloat(el.delay.value) || 0.5 });
  }
});

// Lock/unlock config inputs during crawl
function lockConfigInputs(lock) {
  var inputs = [el.urls, el.outputDir, el.contentFormat, el.maxDepth, el.recursive];
  for (var i = 0; i < inputs.length; i++) {
    inputs[i].disabled = lock;
  }
  el.extensions.querySelectorAll('input').forEach(function(cb) { cb.disabled = lock; });
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
  
  // Load file list from index.json
  var outputDir = el.outputDir.value || './output';
  invoke('read_index', { outputDir: outputDir }).then(function(indexStr) {
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
        var meta = fileTree[name];
        var st = newSet[name] ? 'new' : (updSet[name] ? 'updated' : 'unchanged');
        addFileToList(name, st, meta.source_url || '');
      }
    } catch(e) {
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

// Start crawl
el.startBtn.addEventListener('click', function() {
  if (isRunning) return;
  
  isRunning = true;
  crawledFiles = [];
  el.startBtn.disabled = true;
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
  
  // Register event listener for real-time progress
  var unlisten = null;
  listen('crawl-progress', function(event) {
    var data = event.payload;
    var logType = 'info';
    if (data.status === 'new') logType = 'success';
    else if (data.status === 'updated') logType = 'success';
    else if (data.status === 'error') logType = 'error';
    log(data.line, logType);
    if (data.file_name && data.status !== 'info') {
      addFileToList(data.file_name, data.status, data.url || '');
    }
    el.progressText.textContent = '\u5df2\u5904\u7406 ' + crawledFiles.length + ' \u4e2a\u6587\u4ef6';
    if (data.line.indexOf('Crawling:') !== -1) {
      var crawlUrl = data.line.split('Crawling:')[1];
      if (crawlUrl) el.statusText.textContent = '\u6b63\u5728\u722c\u53d6: ' + crawlUrl.trim();
    }
  }).then(function(fn) { unlisten = fn; });
  
  // Start crawl flow - config is already auto-saved
  log('\u5f00\u59cb\u722c\u53d6...');
  var yaml = toYAML(getConfig());
  
  invoke('write_config', { configPath: configPath, content: yaml }).then(function() {
    var cfg = getConfig();
    for (var u = 0; u < cfg.crawler.urls.length; u++) {
      log('\u76ee\u6807URL: ' + cfg.crawler.urls[u], 'info');
    }
    return invoke('run_crawler', { configPath: configPath });
  }).then(function(result) {
    onCrawlComplete(result);
    if (unlisten) unlisten();
  }, function(e) {
    log('\u722c\u53d6\u5931\u8d25: ' + e, 'error');
    el.statusText.textContent = '\u5931\u8d25';
    resetUI();
    if (unlisten) unlisten();
  });
});

// Stop
el.stopBtn.addEventListener('click', function() {
  log('\u505c\u6b62\u722c\u53d6...');
  resetUI();
  el.statusText.textContent = '\u5df2\u505c\u6b62';
});

// Clear crawl results
document.getElementById('clearBtn').addEventListener('click', function() {
  if (isRunning) {
    log('\u722c\u53d6\u8fdb\u884c\u4e2d\uff0c\u65e0\u6cd5\u6e05\u7a7a', 'error');
    return;
  }
  // Extract domain subdirs from configured URLs
  var urls = el.urls.value.split('\n').filter(function(u) { return u.trim(); });
  var subdirs = [];
  for (var i = 0; i < urls.length; i++) {
    try {
      var a = document.createElement('a');
      a.href = urls[i].trim();
      if (a.hostname) subdirs.push(a.hostname);
    } catch(e) {}
  }
  if (subdirs.length === 0) {
    log('\u6ca1\u6709\u914d\u7f6e\u76ee\u6807URL', 'error');
    return;
  }
  var outputDir = el.outputDir.value || './output';
  log('\u6b63\u5728\u6e05\u7a7a: ' + subdirs.join(', ') + ' ...', 'info');
  invoke('clear_output', { outputDir: outputDir, subdirs: subdirs }).then(function(msg) {
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
    el.previewOpenBtn.style.display = 'none';
  }, function(e) {
    log('\u6e05\u7a7a\u5931\u8d25: ' + e, 'error');
  });
});

// Init
function loadSavedConfig() {
  invoke('read_config', { configPath: configPath }).then(function(yamlStr) {
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
    if (cfg.recursive === 'true') el.recursive.checked = true;
    else if (cfg.recursive === 'false') el.recursive.checked = false;
    if (exts && exts.length) {
      el.extensions.querySelectorAll('input').forEach(function(cb) {
        cb.checked = exts.indexOf(cb.value) !== -1;
      });
    }
    log('\u914d\u7f6e\u5df2\u52a0\u8f7d', 'success');
  }, function() {
    log('\u672a\u627e\u5230\u914d\u7f6e\u6587\u4ef6\uff0c\u4f7f\u7528\u9ed8\u8ba4\u914d\u7f6e', 'info');
  });
}

function loadLastResults() {
  var outputDir = el.outputDir.value || './output';
  invoke('read_index', { outputDir: outputDir }).then(function(indexStr) {
    try {
      var indexData = JSON.parse(indexStr);
      var fileTree = indexData.file_tree || {};
      var names = Object.keys(fileTree);
      if (names.length === 0) return;
      crawledFiles = [];
      el.fileList.innerHTML = '';
      for (var i = 0; i < names.length; i++) {
        var name = names[i];
        var meta = fileTree[name];
        addFileToList(name, 'unchanged', meta.source_url || '');
      }
      el.totalCount.textContent = names.length;
      el.unchangedCount.textContent = names.length;
      el.progressFill.style.width = '100%';
      el.progressText.textContent = '\u4e0a\u6b21\u722c\u53d6: ' + names.length + ' \u4e2a\u6587\u4ef6';
      log('\u5df2\u52a0\u8f7d\u4e0a\u6b21\u722c\u53d6\u7ed3\u679c: ' + names.length + ' \u4e2a\u6587\u4ef6', 'success');
    } catch(e) { /* no previous results */ }
  }, function() { /* no index file yet */ });
}

if (window.__TAURI__ && window.__TAURI__.core) {
  log('DPCrawler \u5df2\u5c31\u7eea', 'success');
  loadSavedConfig();
  // Load last results after a short delay to let config populate outputDir first
  setTimeout(function() { loadLastResults(); }, 200);
} else {
  log('Tauri API \u672a\u68c0\u6d4b\u5230', 'error');
}

}); // end DOMContentLoaded
