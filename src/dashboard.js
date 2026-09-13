// sharecli dashboard — extracted from dashboard.html
// Handles WebSocket connection, process table, operator panels,
// and the three new panels: Rulesets, Logs, Savings.
(function () {
  'use strict';

  const dot = document.getElementById('status-dot');
  const label = document.getElementById('status-label');
  const body = document.getElementById('proc-body');
  const operatorPanels = document.getElementById('operator-panels');
  const lastUpdate = document.getElementById('last-update');
  let reconnectTimer = null;
  let emptyTimer = null;
  let hadProcesses = false;
  let disconnectShown = false;
  const traceparent = document.documentElement.dataset.traceparent || '';

  const ASSET_BASE = '/assets/dashboard/ui';

  const EMPTY_STATES = {
    'first-run': {
      title: 'No processes yet',
      body: 'Start your first harness to see live status in this table.',
      cta: 'sharecli start &lt;project&gt; &lt;harness&gt;',
      href: 'https://github.com/KooshaPari/sharecli#quick-start',
      image: '/assets/dashboard/ui/empty-states/no-data.svg',
      imageAlt: 'No data yet',
    },
    cleared: {
      title: 'All processes stopped',
      body: 'Processes were running earlier but the pool is empty now.',
      cta: 'sharecli ps',
      href: '#main-content',
      image: '/assets/dashboard/ui/empty-states/no-results.svg',
      imageAlt: 'No results',
    },
  };

  function traceHeaders() {
    const headers = {};
    if (traceparent) {
      headers['traceparent'] = traceparent;
    }
    return headers;
  }

  async function probeServe() {
    if (!traceparent) return;
    try {
      await fetch('/healthz', { headers: traceHeaders() });
    } catch (_) {}
  }

  const SKELETON_ROWS = 3;
  const SKELETON_LAYOUT = [
    ['lg', 'sm', 'md', 'lg', 'sm'],
    ['xl', 'sm', 'md', 'md', 'sm'],
    ['lg', 'sm', 'md', 'xl', 'sm'],
  ];
  const OPERATOR_SKELETON_LAYOUT = {
    gate: ['md', 'sm', 'sm', 'lg'],
    host: ['sm', 'md', 'sm', 'xl'],
    pool: ['md', 'md', 'sm', 'sm'],
    status: ['sm', 'sm', 'sm', 'sm'],
    agents: ['sm', 'sm', 'md'],
  };

  function skeletonCell(size) {
    return '<span class="skeleton-bar skeleton-bar--' + size + '"></span>';
  }

  function renderOperatorPanelSkeletons() {
    operatorPanels.setAttribute('aria-busy', 'true');
    var gateDds = ['gate-pressure', 'gate-decision', 'gate-agents', 'gate-contention'];
    gateDds.forEach(function (id, i) {
      var el = document.getElementById(id);
      el.className = '';
      el.innerHTML = skeletonCell(OPERATOR_SKELETON_LAYOUT.gate[i]);
      el.setAttribute('data-loading-kind', 'panel-value');
    });
    var hostDds = ['host-fds', 'host-rss', 'host-load', 'host-net'];
    hostDds.forEach(function (id, i) {
      var el = document.getElementById(id);
      el.innerHTML = skeletonCell(OPERATOR_SKELETON_LAYOUT.host[i]);
      el.setAttribute('data-loading-kind', 'panel-value');
    });
    var poolDds = ['pool-node', 'pool-bun', 'pool-max', 'pool-health'];
    poolDds.forEach(function (id, i) {
      var el = document.getElementById(id);
      el.className = '';
      el.innerHTML = skeletonCell(OPERATOR_SKELETON_LAYOUT.pool[i]);
      el.setAttribute('data-loading-kind', 'panel-value');
    });
    var poolIssues = document.getElementById('pool-issues');
    poolIssues.innerHTML = skeletonCell('lg');
    poolIssues.setAttribute('data-loading-kind', 'panel-value');
    var statusDds = ['status-managed', 'status-scanned', 'status-watched', 'status-agent-rows'];
    statusDds.forEach(function (id, i) {
      var el = document.getElementById(id);
      el.innerHTML = skeletonCell(OPERATOR_SKELETON_LAYOUT.status[i]);
      el.setAttribute('data-loading-kind', 'panel-value');
    });
    var agentDds = ['agents-scanned', 'agents-watched', 'agents-rss'];
    agentDds.forEach(function (id, i) {
      var el = document.getElementById(id);
      el.innerHTML = skeletonCell(OPERATOR_SKELETON_LAYOUT.agents[i]);
      el.setAttribute('data-loading-kind', 'panel-value');
    });
    var families = document.getElementById('agents-families');
    families.innerHTML = skeletonCell('lg');
    families.setAttribute('data-loading-kind', 'panel-value');
  }

  function clearOperatorPanelSkeletons() {
    operatorPanels.removeAttribute('aria-busy');
    document.querySelectorAll('[data-loading-kind="panel-value"]').forEach(function (el) {
      el.removeAttribute('data-loading-kind');
    });
  }

  function renderSkeletonRows() {
    body.setAttribute('aria-busy', 'true');
    body.innerHTML = SKELETON_LAYOUT.map(function (cols, rowIdx) {
      return '<tr class="skeleton-row" data-loading-kind="table-row">' +
        '<td>' + skeletonCell(cols[0]) + '</td>' +
        '<td class="pid">' + skeletonCell(cols[1]) + '</td>' +
        '<td class="mem">' + skeletonCell(cols[2]) + '</td>' +
        '<td>' + skeletonCell(cols[3]) + '</td>' +
        '<td class="status">' + skeletonCell(cols[4]) + '</td>' +
        '</tr>';
    }).join('');
  }

  function setConnecting(active) {
    dot.classList.toggle('connecting', active);
  }

  function connect() {
    setConnecting(true);
    label.textContent = 'connecting\u2026';
    renderOperatorPanelSkeletons();
    renderSkeletonRows();
    var ws = new WebSocket('ws://localhost:9000/ws');

    ws.onopen = function () {
      dot.classList.remove('connecting');
      dot.classList.add('connected');
      label.textContent = 'connected \u2014 loading processes\u2026';
      disconnectShown = false;
      if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
      if (emptyTimer) { clearTimeout(emptyTimer); }
      emptyTimer = setTimeout(function () {
        emptyTimer = null;
        if (document.getElementById('loading-row')) {
          renderEmptyState('first-run');
        }
      }, 300);
    };

    ws.onmessage = function (evt) {
      if (emptyTimer) { clearTimeout(emptyTimer); emptyTimer = null; }
      var data;
      try { data = JSON.parse(evt.data); } catch (_) { return; }
      if (data.event === 'thermal_warning') {
        setThermal('warning', '\u26a0\ufe0f Warning');
        return;
      }
      if (data.event === 'thermal_critical') {
        setThermal('critical', '\U0001f534 Critical');
        return;
      }
      if (data.gate || data.host_watch || data.pool || data.status || data.agents) {
        renderOperatorPanels(data);
      }
      var procs = Array.isArray(data.processes) ? data.processes : [];
      renderTable(procs);
      renderRulesetsPanel(data);
      renderLogsPanel(data);
      renderSavingsPanel(data);
      lastUpdate.textContent = 'last update: ' + new Date().toLocaleTimeString();
    };

    ws.onclose = ws.onerror = function () {
      dot.classList.remove('connected');
      label.textContent = 'disconnected \u2014 reconnecting in 3s';
      renderDisconnectError();
      if (!reconnectTimer) {
        reconnectTimer = setTimeout(function () { reconnectTimer = null; connect(); }, 3000);
      }
    };
  }

  function renderDisconnectError() {
    if (disconnectShown) return;
    disconnectShown = true;
    clearOperatorPanelSkeletons();
    body.removeAttribute('aria-busy');
    body.innerHTML = '<tr><td colspan="5">' +
      '<div class="error-state" role="alert" data-error-kind="disconnect" aria-live="assertive">' +
      '<div class="error-state-icon" aria-hidden="true">' +
      '<img src="/assets/dashboard/ui/error-states/disconnect.svg" alt="" width="240" height="180" decoding="async" data-illustration="disconnect-tier1">' +
      '</div>' +
      '<div class="error-state-title">Dashboard disconnected</div>' +
      '<p class="error-state-body">' +
      'Cannot reach the WebSocket feed. Confirm <code>sharecli serve</code> is running on this host.' +
      '</p>' +
      '<div class="error-state-actions">' +
      '<button type="button" class="error-state-cta" id="error-retry">Retry now</button>' +
      '<code class="error-state-hint">sharecli serve --bind 127.0.0.1:9000</code>' +
      '</div>' +
      '</div>' +
      '</td></tr>';
    var retry = document.getElementById('error-retry');
    if (retry) {
      retry.addEventListener('click', function () {
        if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
        disconnectShown = false;
        connect();
      });
    }
  }

  function renderEmptyState(kind) {
    body.removeAttribute('aria-busy');
    label.textContent = 'connected';
    var spec = EMPTY_STATES[kind] || EMPTY_STATES['first-run'];
    body.innerHTML = '<tr><td colspan="5">' +
      '<div class="empty-state" role="status" data-empty-kind="' + kind + '" aria-live="polite">' +
      '<div class="empty-state-icon" aria-hidden="true">' +
      '<img src="' + spec.image + '" alt="' + spec.imageAlt + '" width="240" height="180" decoding="async">' +
      '</div>' +
      '<div class="empty-state-title">' + spec.title + '</div>' +
      '<p class="empty-state-body">' + spec.body + '</p>' +
      '<code class="empty-state-cta" role="button" tabindex="0">' + spec.cta + '</code>' +
      '</div>' +
      '</td></tr>';
  }

  function renderTable(procs) {
    body.removeAttribute('aria-busy');
    label.textContent = 'connected';
    if (procs.length === 0) {
      var kind = hadProcesses ? 'cleared' : 'first-run';
      renderEmptyState(kind);
      return;
    }
    hadProcesses = true;
    body.innerHTML = procs.map(function (p) {
      var name    = esc(p.name    || '\u2014');
      var pid     = esc(String(p.pid || '\u2014'));
      var mem     = p.memory_mb != null ? Number(p.memory_mb).toFixed(1) : '\u2014';
      var project = esc(p.project || '\u2014');
      var status  = 'running';
      return '<tr>' +
        '<td>' + name + '</td>' +
        '<td class="pid">' + pid + '</td>' +
        '<td class="mem">' + mem + '</td>' +
        '<td>' + project + '</td>' +
        '<td class="status">' + status + '</td>' +
        '</tr>';
    }).join('');
  }

  var thermalEl = document.getElementById('thermal-status');

  function formatBytes(n) {
    if (n == null || Number.isNaN(Number(n))) return '\u2014';
    var v = Number(n);
    if (v < 1024) return v + ' B';
    if (v < 1024 * 1024) return (v / 1024).toFixed(1) + ' KiB';
    if (v < 1024 * 1024 * 1024) return (v / (1024 * 1024)).toFixed(1) + ' MiB';
    return (v / (1024 * 1024 * 1024)).toFixed(2) + ' GiB';
  }

  function renderOperatorPanels(data) {
    clearOperatorPanelSkeletons();
    var gate = data.gate || {};
    var host = data.host_watch || {};
    var agents = data.agents || {};

    var pressure = gate.thermal_pressure || '\u2014';
    document.getElementById('gate-pressure').textContent = pressure;
    var decisionEl = document.getElementById('gate-decision');
    var decision = gate.gate_decision || '\u2014';
    decisionEl.textContent = decision;
    decisionEl.className = decision === 'ADMIT' ? 'gate-admit'
      : decision === 'DENY' ? 'gate-deny' : 'gate-unavailable';
    document.getElementById('gate-agents').textContent =
      gate.detected_agents != null ? String(gate.detected_agents) : '\u2014';
    document.getElementById('gate-contention').textContent = gate.agent_contention || '\u2014';

    document.getElementById('host-fds').textContent =
      host.fd_count != null ? String(host.fd_count) : '\u2014';
    document.getElementById('host-rss').textContent = formatBytes(host.mem_rss_bytes);
    document.getElementById('host-load').textContent =
      host.load_1m != null ? Number(host.load_1m).toFixed(2) : '\u2014';
    var rx = host.net_rx_bytes != null ? formatBytes(host.net_rx_bytes) : '\u2014';
    var tx = host.net_tx_bytes != null ? formatBytes(host.net_tx_bytes) : '\u2014';
    document.getElementById('host-net').textContent = rx + ' / ' + tx;

    var pool = data.pool || {};
    document.getElementById('pool-node').textContent =
      (pool.node_total != null && pool.node_idle != null)
        ? pool.node_idle + '/' + pool.node_total + ' idle'
        : '\u2014';
    document.getElementById('pool-bun').textContent =
      (pool.bun_total != null && pool.bun_idle != null)
        ? pool.bun_idle + '/' + pool.bun_total + ' idle'
        : '\u2014';
    document.getElementById('pool-max').textContent =
      pool.max_per_type != null ? String(pool.max_per_type) : '\u2014';
    var poolHealthEl = document.getElementById('pool-health');
    if (pool.healthy === true) {
      poolHealthEl.textContent = 'healthy';
      poolHealthEl.className = 'gate-admit';
    } else if (pool.healthy === false) {
      poolHealthEl.textContent = 'degraded';
      poolHealthEl.className = 'gate-deny';
    } else {
      poolHealthEl.textContent = '\u2014';
      poolHealthEl.className = '';
    }
    var issues = Array.isArray(pool.issues) ? pool.issues : [];
    document.getElementById('pool-issues').textContent = issues.length
      ? issues.join(' \u00b7 ')
      : 'no issues';

    var status = data.status || {};
    document.getElementById('status-managed').textContent =
      status.total_processes != null ? String(status.total_processes) : '\u2014';
    document.getElementById('status-scanned').textContent =
      status.scanned != null ? String(status.scanned) : '\u2014';
    document.getElementById('status-watched').textContent =
      status.watched != null ? String(status.watched) : '\u2014';
    var agentRows = Array.isArray(status.agents) ? status.agents.length : null;
    document.getElementById('status-agent-rows').textContent =
      agentRows != null ? String(agentRows) : '\u2014';

    document.getElementById('agents-scanned').textContent =
      agents.scanned != null ? String(agents.scanned) : '\u2014';
    document.getElementById('agents-watched').textContent =
      agents.watched != null ? String(agents.watched) : '\u2014';
    document.getElementById('agents-rss').textContent = formatBytes(agents.total_rss_bytes);

    var families = agents.families || {};
    var famKeys = Object.keys(families);
    document.getElementById('agents-families').textContent = famKeys.length
      ? famKeys.map(function (k) { return k + ':' + families[k]; }).join(' \u00b7 ')
      : 'no families';

    if (pressure === 'GREEN') setThermal('', 'Normal');
    else if (pressure === 'YELLOW') setThermal('warning', 'Warning');
    else if (pressure === 'RED') setThermal('critical', 'Critical');
    else if (pressure === 'UNAVAILABLE') setThermal('warning', 'Unavailable');
  }

  function setThermal(cls, text) {
    thermalEl.className = cls;
    thermalEl.textContent = text;
  }

  function esc(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // ---------------------------------------------------------------------------
  // Rulesets panel — process grouping rules derived from pool/config data
  // ---------------------------------------------------------------------------

  function renderRulesetsPanel(data) {
    var el = document.getElementById('rulesets-body');
    if (!el) return;
    var pool = data.pool || {};
    var status = data.status || {};
    var gate = data.gate || {};

    var rules = [];

    // Pool type grouping rule
    if (pool.node_total != null || pool.bun_total != null) {
      var types = [];
      if (pool.node_total != null) types.push('node (' + pool.node_total + ')');
      if (pool.bun_total != null) types.push('bun (' + pool.bun_total + ')');
      rules.push({
        name: 'Runtime grouping',
        pattern: types.join(', '),
        policy: pool.healthy === true ? 'healthy' : pool.healthy === false ? 'degraded' : 'unknown',
        max: pool.max_per_type != null ? String(pool.max_per_type) : '\u2014',
      });
    }

    // Thermal gate policy
    var gateDecision = gate.gate_decision || '\u2014';
    rules.push({
      name: 'Thermal gate',
      pattern: 'all processes',
      policy: gateDecision,
      max: gate.detected_agents != null ? String(gate.detected_agents) + ' agents' : '\u2014',
    });

    // Process limit
    if (status.total_processes != null) {
      rules.push({
        name: 'Process limit',
        pattern: 'managed pids',
        policy: status.scanned != null ? status.scanned + ' scanned' : '\u2014',
        max: String(status.total_processes),
      });
    }

    // Watched agents
    if (status.watched != null && status.watched > 0) {
      rules.push({
        name: 'Agent watch',
        pattern: 'host agents',
        policy: 'watching',
        max: String(status.watched),
      });
    }

    if (rules.length === 0) {
      el.innerHTML = '<tr><td colspan="4" class="empty-state" style="padding:16px;border:none">' +
        'No rulesets active \u2014 waiting for data</td></tr>';
      return;
    }

    el.innerHTML = rules.map(function (r) {
      var policyClass = r.policy === 'ADMIT' || r.policy === 'healthy' || r.policy === 'watching'
        ? 'gate-admit'
        : r.policy === 'DENY' || r.policy === 'degraded'
          ? 'gate-deny'
          : 'gate-unavailable';
      return '<tr>' +
        '<td>' + esc(r.name) + '</td>' +
        '<td class="pid">' + esc(r.pattern) + '</td>' +
        '<td class="' + policyClass + '">' + esc(r.policy) + '</td>' +
        '<td class="mem">' + esc(r.max) + '</td>' +
        '</tr>';
    }).join('');
  }

  // ---------------------------------------------------------------------------
  // Logs panel — recent actions (kills, restarts, spawns) via WebSocket feed
  // The backend can send { logs: [{ ts, level, action, detail }] }
  // ---------------------------------------------------------------------------

  var logEntries = [];
  var LOG_MAX = 200;
  var logFilter = '';

  function renderLogsPanel(data) {
    var el = document.getElementById('logs-body');
    var countEl = document.getElementById('logs-count');
    if (!el) return;

    // Accept logs from the WebSocket message
    if (Array.isArray(data.logs)) {
      data.logs.forEach(function (entry) {
        logEntries.unshift(entry);
      });
      if (logEntries.length > LOG_MAX) {
        logEntries.length = LOG_MAX;
      }
    }

    var filtered = logEntries;
    if (logFilter) {
      var needle = logFilter.toLowerCase();
      filtered = logEntries.filter(function (e) {
        return (e.action || '').toLowerCase().indexOf(needle) !== -1 ||
               (e.detail || '').toLowerCase().indexOf(needle) !== -1;
      });
    }

    if (countEl) {
      countEl.textContent = filtered.length + ' entries';
    }

    if (filtered.length === 0) {
      el.innerHTML = '<tr><td colspan="4" class="empty-state" style="padding:16px;border:none">' +
        'No log entries yet \u2014 actions will appear here in real time</td></tr>';
      return;
    }

    el.innerHTML = filtered.map(function (e) {
      var ts = e.ts || '';
      var level = (e.level || 'info').toUpperCase();
      var action = esc(e.action || '\u2014');
      var detail = esc(e.detail || '');
      var lvlClass = level === 'ERROR' ? 'gate-deny'
        : level === 'WARN' ? 'gate-unavailable'
        : 'gate-admit';
      return '<tr>' +
        '<td class="pid">' + esc(ts) + '</td>' +
        '<td class="' + lvlClass + '">' + level + '</td>' +
        '<td>' + action + '</td>' +
        '<td class="pid">' + detail + '</td>' +
        '</tr>';
    }).join('');
  }

  // Log filter input handler
  var logFilterEl = document.getElementById('logs-filter');
  if (logFilterEl) {
    logFilterEl.addEventListener('input', function () {
      logFilter = logFilterEl.value;
      renderLogsPanel({});
    });
  }

  // ---------------------------------------------------------------------------
  // Savings panel — resource savings from coalescing and pool effectiveness
  // The backend can send { savings: { ... } } or we derive from pool data
  // ---------------------------------------------------------------------------

  function renderSavingsPanel(data) {
    var el = document.getElementById('savings-body');
    if (!el) return;

    var pool = data.pool || {};
    var savings = data.savings || {};
    var agents = data.agents || {};

    // Derive savings metrics from available data
    var nodeIdle = pool.node_total != null && pool.node_idle != null ? pool.node_total - pool.node_idle : null;
    var bunIdle = pool.bun_total != null && pool.bun_idle != null ? pool.bun_total - pool.bun_idle : null;
    var totalProcesses = pool.node_total != null && pool.bun_total != null ? pool.node_total + pool.bun_total : null;
    var totalIdle = nodeIdle != null && bunIdle != null ? nodeIdle + bunIdle : null;
    var poolEfficiency = totalProcesses != null && totalProcesses > 0
      ? ((totalIdle / totalProcesses) * 100).toFixed(1)
      : '\u2014';

    // Estimated RSS saved by pooling (idle slots = processes not spawned)
    var savedRssBytes = savings.saved_rss_bytes || agents.total_rss_bytes || null;
    var hitRate = savings.hit_rate_pct || null;
    var avoidedSpawns = savings.avoided_spawns || null;

    var metrics = [
      { label: 'Pool efficiency', value: poolEfficiency !== '\u2014' ? poolEfficiency + '%' : '\u2014',
        desc: 'idle / total slots', color: poolEfficiency !== '\u2014' && Number(poolEfficiency) > 50
          ? 'gate-admit' : 'gate-unavailable' },
      { label: 'Active processes', value: totalProcesses != null ? String(totalProcesses) : '\u2014',
        desc: 'node + bun in pool', color: '' },
      { label: 'Idle slots', value: totalIdle != null ? String(totalIdle) : '\u2014',
        desc: 'available without spawn', color: totalIdle != null && totalIdle > 0 ? 'gate-admit' : '' },
      { label: 'Estimated RSS saved',
        value: savedRssBytes != null ? formatBytes(savedRssBytes) : '\u2014',
        desc: 'memory avoided by coalescing', color: 'mem' },
      { label: 'Coalesce hit rate',
        value: hitRate != null ? Number(hitRate).toFixed(1) + '%' : '\u2014',
        desc: 'cache hits / total lookups', color: hitRate != null && Number(hitRate) > 70
          ? 'gate-admit' : hitRate != null ? 'gate-unavailable' : '' },
      { label: 'Avoided spawns',
        value: avoidedSpawns != null ? String(avoidedSpawns) : '\u2014',
        desc: 'processes not re-created', color: avoidedSpawns != null && avoidedSpawns > 0
          ? 'gate-admit' : '' },
    ];

    el.innerHTML = metrics.map(function (m) {
      return '<div class="savings-metric">' +
        '<dt class="savings-label">' + m.label + '</dt>' +
        '<dd class="savings-value ' + m.color + '">' + m.value + '</dd>' +
        '<dd class="savings-desc">' + m.desc + '</dd>' +
        '</div>';
    }).join('');
  }

  // ---------------------------------------------------------------------------
  // Boot
  // ---------------------------------------------------------------------------

  connect();
  probeServe();
})();
