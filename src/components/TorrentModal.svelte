<script>
  let { animeTitle = '', episodeNumber = null, onClose, onSelect } = $props();

  async function tauri(cmd, args = {}) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke(cmd, args);
  }

  let results      = $state([]);
  let loading      = $state(true);
  let error        = $state('');
  let selected     = $state(null);
  let fileInfo     = $state(null);
  let loadingFiles = $state(false);
  let activeTab    = $state('nyaa'); // 'animetosho' | 'nyaa'

  const epStr = episodeNumber != null ? String(episodeNumber) : null;
  const epPadded = epStr != null
    ? epStr.padStart(epStr.length <= 2 ? 2 : epStr.length <= 3 ? 3 : 4, '0')
    : null;
  const autoQuery = epPadded != null ? `${animeTitle} ${epPadded}` : animeTitle;

  let manualQuery = $state(autoQuery);

  $effect(() => { search(autoQuery, activeTab); });

  async function search(q, tab) {
    loading = true; error = ''; results = []; selected = null; fileInfo = null;
    try {
      let res;
      if (tab === 'nyaa') {
        res = await tauri('nyaa_direct', { query: q.trim() });
        // Fallback: título sin ':' (e.g. "Jujutsu Kaisen Shimetsu 23")
        if (res.length === 0 && q.includes(':'))
          res = await tauri('nyaa_direct', { query: q.replace(/:/g, '').replace(/\s+/g, ' ').trim() });
      } else {
        res = await tauri('nyaa_search', { query: q.trim(), category: '1_2' });
        if (res.length === 0 && q.includes(':'))
          res = await tauri('nyaa_search', { query: q.replace(/:/g, '').replace(/\s+/g, ' ').trim(), category: '1_2' });
      }
      results = res;
      if (results.length === 0) error = 'Sin resultados. Prueba buscar manualmente, p.ej. "Jujutsu Kaisen S3 23"';
    } catch(e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function switchTab(tab) {
    if (tab === activeTab) return;
    activeTab = tab;
    search(manualQuery, tab);
  }

  async function selectTorrent(r) {
    selected = r;
    fileInfo = null;
    loadingFiles = true;
    try {
      fileInfo = await tauri('nyaa_torrent_info', { torrentUrl: r.torrent });
    } catch {
      fileInfo = [];
    } finally {
      loadingFiles = false;
    }
  }

  function chooseFile(f) { onSelect({ torrent: selected, file: f }); onClose(); }
  function chooseDirectly() { onSelect({ torrent: selected, file: null }); onClose(); }

  function formatSize(bytes) {
    if (!bytes) return '';
    if (bytes > 1e9) return (bytes / 1e9).toFixed(1) + ' GB';
    return (bytes / 1e6).toFixed(0) + ' MB';
  }

  function qualityTag(title) {
    if (/1080p/i.test(title)) return '1080p';
    if (/720p/i.test(title))  return '720p';
    if (/480p/i.test(title))  return '480p';
    return '';
  }

  function groupTag(title) {
    const m = title.match(/^\[([^\]]+)\]/);
    return m ? m[1] : '';
  }

  function isMultiSub(title) {
    return /\bmulti[-\s]?sub/i.test(title) || /\[multi\]/i.test(title);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="panel">
    <div class="panel-header">
      <div class="panel-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <polyline points="8 17 12 21 16 17"/><line x1="12" y1="3" x2="12" y2="21"/>
          <polyline points="20 7 12 3 4 7"/>
        </svg>
        {animeTitle}
        {#if episodeNumber != null}<span class="ep-badge">Ep. {episodeNumber}</span>{/if}
      </div>
      <button class="close-btn" onclick={onClose}>✕</button>
    </div>

    <!-- Tabs -->
    <div class="tabs">
      <button class="tab" class:active={activeTab === 'nyaa'} onclick={() => switchTab('nyaa')}>
        Nyaa.si
      </button>
      <button class="tab" class:active={activeTab === 'animetosho'} onclick={() => switchTab('animetosho')}>
        AnimeToSho
      </button>
    </div>

    <div class="search-bar">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input
        type="text"
        bind:value={manualQuery}
        onkeydown={(e) => e.key === 'Enter' && search(manualQuery, activeTab)}
        placeholder="Buscar torrents…"
      />
      <button onclick={() => search(manualQuery, activeTab)}>Buscar</button>
    </div>

    <div class="body">
      {#if loading}
        <div class="centered">
          <div class="spinner"></div>
          <span>Buscando torrents…</span>
        </div>
      {:else if error}
        <div class="centered error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          <span>{error}</span>
        </div>
      {:else if selected}
        <div class="file-view">
          <button class="back-link" onclick={() => { selected = null; fileInfo = null; }}>← Volver</button>
          <div class="sel-title">{selected.title}</div>
          {#if loadingFiles}
            <div class="centered"><div class="spinner"></div><span>Leyendo archivos…</span></div>
          {:else if fileInfo && fileInfo.length > 0}
            <p class="files-hint">Elige el archivo de vídeo:</p>
            <div class="files-list">
              {#each fileInfo as f (f.path)}
                <button class="file-row" onclick={() => chooseFile(f)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>
                  <span class="file-name">{f.name}</span>
                  <span class="file-size">{formatSize(f.size)}</span>
                </button>
              {/each}
            </div>
          {/if}
          <button class="btn-play" onclick={chooseDirectly}>
            <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            Reproducir torrent completo
          </button>
        </div>
      {:else}
        <div class="results-list">
          {#each results as r (r.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="result-row" class:multi={isMultiSub(r.title)} onclick={() => selectTorrent(r)}>
              <div class="result-left">
                {#if isMultiSub(r.title)}<span class="tag multi-badge">Multi</span>{/if}
                {#if groupTag(r.title)}<span class="tag group">{groupTag(r.title)}</span>{/if}
                {#if qualityTag(r.title)}<span class="tag quality">{qualityTag(r.title)}</span>{/if}
                <span class="result-title">{r.title.replace(/^\[[^\]]+\]\s*/, '')}</span>
              </div>
              <div class="result-right">
                <span class="seed">▲{r.seeders}</span>
                <span class="leech">▼{r.leechers}</span>
                {#if r.size}<span class="size">{r.size}</span>{/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 9999;
    background: rgba(0,0,0,0.75);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(4px);
    padding: 20px;
  }

  .panel {
    width: min(680px, 100%);
    max-height: 80vh;
    background: var(--bg, #0f0f0f);
    border: 1px solid var(--outline-dim, rgba(255,255,255,0.1));
    border-radius: 14px;
    display: flex; flex-direction: column;
    overflow: hidden;
    animation: fadein 0.2s ease;
    box-shadow: 0 20px 60px rgba(0,0,0,0.5);
  }
  @keyframes fadein { from { transform: scale(0.96); opacity: 0; } to { transform: none; opacity: 1; } }

  .panel-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .panel-title {
    display: flex; align-items: center; gap: 8px;
    font-size: 13px; font-weight: 700; color: var(--text);
    min-width: 0;
  }
  .panel-title svg { width: 15px; height: 15px; color: var(--primary, #f59e0b); flex-shrink: 0; }

  .ep-badge {
    font-size: 10px; font-weight: 700;
    background: rgba(245,158,11,0.15); color: var(--primary, #f59e0b);
    padding: 2px 7px; border-radius: 5px; flex-shrink: 0;
  }

  .close-btn {
    background: none; border: none; color: var(--text-muted);
    font-size: 14px; cursor: pointer; padding: 4px 8px; border-radius: 4px; flex-shrink: 0;
  }

  /* ── Tabs ── */
  .tabs {
    display: flex; gap: 0;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .tab {
    flex: 1; padding: 9px 0; border: none; background: none;
    color: var(--text-muted); font-size: 12px; font-weight: 600;
    cursor: pointer; border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--primary, #f59e0b); border-bottom-color: var(--primary, #f59e0b); }

  .body { flex: 1; overflow-y: auto; }

  .centered {
    display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 10px;
    color: var(--text-muted); font-size: 12px;
    padding: 40px 20px;
  }
  .centered.error { color: #f87171; }
  .centered svg { width: 24px; height: 24px; }

  .results-list { display: flex; flex-direction: column; }

  .result-row {
    display: flex; align-items: center; justify-content: space-between;
    gap: 10px; padding: 10px 16px; cursor: pointer;
    border-bottom: 1px solid var(--outline-dim, rgba(255,255,255,0.05));
    transition: background 0.1s;
  }
  .result-row:hover { background: var(--bg-card, rgba(255,255,255,0.04)); }

  .result-left {
    display: flex; align-items: center; gap: 6px;
    min-width: 0; flex: 1;
  }

  .result-title {
    font-size: 11px; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .tag {
    font-size: 9px; font-weight: 800;
    padding: 1px 5px; border-radius: 3px;
    flex-shrink: 0; white-space: nowrap;
  }
  .tag.group      { background: rgba(99,102,241,0.15); color: #818cf8; }
  .tag.quality    { background: rgba(16,185,129,0.15); color: #34d399; }
  .tag.multi-badge { background: rgba(245,158,11,0.2); color: #f59e0b; }
  .result-row.multi { border-left: 2px solid rgba(245,158,11,0.4); }

  .result-right {
    display: flex; align-items: center; gap: 10px;
    font-size: 10px; flex-shrink: 0;
  }
  .seed  { color: #4ade80; font-weight: 700; }
  .leech { color: #f87171; }
  .size  { color: var(--text-muted); }

  /* ── File picker ── */
  .file-view { padding: 14px 16px; display: flex; flex-direction: column; gap: 10px; }

  .back-link {
    background: none; border: none; color: var(--primary, #f59e0b);
    font-size: 11px; font-weight: 600; cursor: pointer; padding: 0; text-align: left;
  }

  .sel-title {
    font-size: 11px; font-weight: 600; color: var(--text);
    background: var(--bg-card, rgba(255,255,255,0.05));
    border-radius: 6px; padding: 8px 10px; word-break: break-all;
  }

  .files-hint { font-size: 11px; color: var(--text-muted); margin: 0; }
  .files-list { display: flex; flex-direction: column; gap: 4px; }

  .file-row {
    display: flex; align-items: center; gap: 8px;
    background: var(--bg-card, rgba(255,255,255,0.04));
    border: 1px solid var(--outline-dim); border-radius: 6px;
    padding: 8px 12px; cursor: pointer; text-align: left; width: 100%;
    transition: border-color 0.12s;
  }
  .file-row:hover { border-color: var(--primary, #f59e0b); }
  .file-row svg { width: 14px; height: 14px; flex-shrink: 0; color: var(--primary, #f59e0b); }
  .file-name { flex: 1; font-size: 11px; font-weight: 600; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .file-size { font-size: 10px; color: var(--text-muted); flex-shrink: 0; }

  .btn-play {
    display: flex; align-items: center; justify-content: center; gap: 8px;
    background: var(--primary, #f59e0b); color: #000;
    font-size: 12px; font-weight: 700; border: none; border-radius: 8px;
    padding: 10px 16px; cursor: pointer; width: 100%;
  }
  .btn-play svg { width: 14px; height: 14px; }

  .spinner {
    width: 22px; height: 22px;
    border: 3px solid var(--outline-dim);
    border-top-color: var(--primary, #f59e0b);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .search-bar {
    display: flex; align-items: center; gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }
  .search-bar svg { width: 13px; height: 13px; color: var(--text-muted); flex-shrink: 0; }
  .search-bar input {
    flex: 1; background: var(--bg-card); border: 1px solid var(--outline);
    border-radius: 6px; padding: 5px 8px;
    color: var(--text); font-size: 11px; outline: none;
  }
  .search-bar input:focus { border-color: var(--primary); }
  .search-bar button {
    background: var(--primary); color: #000;
    border: none; border-radius: 6px; padding: 5px 10px;
    font-size: 11px; font-weight: 700; cursor: pointer; flex-shrink: 0;
  }
</style>
