<script>
  // Vista "Mangas" — autocontenida (estado propio, no usa el store de Manwhas).
  // Fuentes en pestañas arriba; de momento solo MangaDex está operativa.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import MangaCard from './MangaCard.svelte';
  import { library, toggleLibrary, downloadPath, downloadFormat, pendingMangadex } from '../stores/manga.js';

  function tauri(cmd, args) { return invoke(cmd, args ?? {}); }

  // ── Fuentes (pestañas superiores, estilo Manwhas) ─────────────────────────
  const SOURCES = [
    { id: 'mangadex', label: 'MangaDex', ready: true,
      cmds: { latest: 'mangadex_latest', browse: 'mangadex_browse', search: 'mangadex_search', info: 'mangadex_info', pages: 'mangadex_pages' } },
  ];
  let activeSource = $state('mangadex');
  let source = $derived(SOURCES.find(s => s.id === activeSource) ?? SOURCES[0]);

  // ── Estado de navegación ──────────────────────────────────────────────────
  let subView = $state('browse'); // 'browse' | 'detail' | 'reader'
  let activeTab = $state('browse'); // 'latest' | 'browse' (Recientes | Populares)
  let loading = $state(false);
  let error   = $state('');
  let items   = $state([]);
  let page    = $state(1);
  let hasMore = $state(false);
  let loadingMore = $state(false);

  let searchQuery = $state('');
  let searchTimer = null;

  // ── Browse ────────────────────────────────────────────────────────────────
  async function load(tab, p = 1, append = false) {
    if (!source.ready) { items = []; loading = false; error = ''; return; }
    if (append) loadingMore = true; else { loading = true; items = []; }
    error = '';
    try {
      const cmd = tab === 'latest' ? source.cmds.latest : source.cmds.browse;
      const r = await tauri(cmd, { page: p });
      const next = r.results ?? [];
      items = append ? [...items, ...next] : next;
      hasMore = r.hasMore ?? false;
      page = p;
    } catch(e) { error = String(e); }
    finally { loading = false; loadingMore = false; }
  }

  async function switchTab(tab) {
    activeTab = tab; searchQuery = ''; await load(tab, 1);
  }

  async function loadMore() { await load(activeTab, page + 1, true); }

  async function doSearch(q) {
    if (!source.ready) return;
    loading = true; error = ''; hasMore = false;
    try {
      const r = await tauri(source.cmds.search, { query: q });
      items = r.results ?? [];
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  function handleSearch() {
    clearTimeout(searchTimer);
    if (!searchQuery.trim()) { switchTab(activeTab); return; }
    const q = searchQuery.trim();
    searchTimer = setTimeout(() => doSearch(q), 450);
  }
  function clearSearch() { searchQuery = ''; switchTab(activeTab); }

  async function switchSource(id) {
    if (id === activeSource) return;
    activeSource = id;
    subView = 'browse'; activeTab = 'browse'; searchQuery = '';
    items = []; page = 1; hasMore = false; error = '';
    await load('browse', 1);
  }

  onMount(() => load('browse', 1));

  // ── Detalle ───────────────────────────────────────────────────────────────
  let detail = $state(null);
  let readSet = $state(new Set());
  let downloadedSet = $state(new Set());
  let chapterState = $state({});           // { [chapterId]: 'loading'|'done'|'error' }
  let downloadAllState = $state('idle');   // 'idle'|'loading'|'done'|'error'
  let downloadAllProgress = $state(0);
  let lastError = $state('');

  let inLib = $derived(!!detail && $library.some(e => e.id === detail.id));
  let allRead = $derived(
    (detail?.chapters?.length ?? 0) > 0 && (detail?.chapters ?? []).every(c => readSet.has(c.id))
  );

  function loadRead(mangaId) {
    try { const raw = localStorage.getItem(`foundry_read_${mangaId}`); return raw ? new Set(JSON.parse(raw)) : new Set(); }
    catch { return new Set(); }
  }
  function saveRead(mangaId, ids) {
    try { localStorage.setItem(`foundry_read_${mangaId}`, JSON.stringify([...ids])); } catch {}
  }
  function loadDownloaded(mangaId) {
    try { const raw = localStorage.getItem(`foundry_dl_${mangaId}`); return raw ? new Set(JSON.parse(raw)) : new Set(); }
    catch { return new Set(); }
  }
  function saveDownloaded(mangaId, ids) {
    try { localStorage.setItem(`foundry_dl_${mangaId}`, JSON.stringify([...ids])); } catch {}
  }

  function markAllRead() {
    if (!detail?.id) return;
    readSet = new Set((detail.chapters ?? []).map(c => c.id));
    saveRead(detail.id, readSet);
  }

  function isDone(chapterId) {
    return chapterState[chapterId] === 'done' || downloadedSet.has(chapterId);
  }

  async function downloadChapter(chapter, e) {
    e.stopPropagation();
    if (chapterState[chapter.id] === 'loading' || isDone(chapter.id)) return;
    chapterState = { ...chapterState, [chapter.id]: 'loading' };
    try {
      await tauri('download_chapter', {
        mangaId: detail.id, mangaTitle: detail.title,
        chapterId: chapter.id, chapterTitle: chapter.title,
        downloadPath: $downloadPath || null,
        format: downloadFormat(),
      });
      chapterState = { ...chapterState, [chapter.id]: 'done' };
      downloadedSet = new Set([...downloadedSet, chapter.id]);
      saveDownloaded(detail.id, downloadedSet);
    } catch(err) {
      lastError = typeof err === 'string' ? err : (err?.message ?? String(err));
      chapterState = { ...chapterState, [chapter.id]: 'error' };
    }
  }

  async function downloadAll(e) {
    e.stopPropagation();
    if (downloadAllState === 'loading') return;
    const chapters = detail?.chapters ?? [];
    if (!chapters.length || !detail?.id) return;
    downloadAllState = 'loading'; downloadAllProgress = 0;
    for (const chapter of chapters) {
      if (isDone(chapter.id)) { downloadAllProgress++; continue; }
      chapterState = { ...chapterState, [chapter.id]: 'loading' };
      try {
        await tauri('download_chapter', {
          mangaId: detail.id, mangaTitle: detail.title,
          chapterId: chapter.id, chapterTitle: chapter.title,
          downloadPath: $downloadPath || null,
        });
        chapterState = { ...chapterState, [chapter.id]: 'done' };
        downloadedSet = new Set([...downloadedSet, chapter.id]);
        saveDownloaded(detail.id, downloadedSet);
      } catch { chapterState = { ...chapterState, [chapter.id]: 'error' }; }
      downloadAllProgress++;
    }
    downloadAllState = downloadAllProgress === chapters.length ? 'done' : 'error';
  }

  async function openManga(item) {
    subView = 'detail';
    detail = { ...item, chapters: [], description: '', genres: [], authors: [] };
    readSet = loadRead(item.id);
    downloadedSet = loadDownloaded(item.id);
    chapterState = {}; downloadAllState = 'idle'; downloadAllProgress = 0; lastError = '';
    loading = true; error = '';
    try {
      detail = await tauri(source.cmds.info, { id: item.id });
      readSet = loadRead(detail.id);
      downloadedSet = loadDownloaded(detail.id);
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  // Abrir un item de MangaDex enviado desde la Biblioteca
  $effect(() => {
    const p = $pendingMangadex;
    if (p) { pendingMangadex.set(null); activeSource = 'mangadex'; openManga(p); }
  });

  // ── Lector ────────────────────────────────────────────────────────────────
  let pages = $state([]);
  let chapterIdx = $state(-1);
  let readerLoading = $state(false);

  let currentChapter = $derived(chapterIdx >= 0 ? detail?.chapters?.[chapterIdx] : null);
  // capítulos en orden desc (como vienen): idx+1 = anterior, idx-1 = siguiente
  let prevChapter = $derived(detail && chapterIdx >= 0 && chapterIdx < (detail.chapters.length - 1) ? detail.chapters[chapterIdx + 1] : null);
  let nextChapter = $derived(detail && chapterIdx > 0 ? detail.chapters[chapterIdx - 1] : null);

  async function openChapter(idx) {
    if (idx < 0 || !detail?.chapters?.[idx]) return;
    chapterIdx = idx;
    subView = 'reader';
    pages = []; readerLoading = true; error = '';
    const ch = detail.chapters[idx];
    try {
      pages = await tauri(source.cmds.pages, { chapterId: ch.id });
      readSet = new Set([...readSet, ch.id]);
      saveRead(detail.id, readSet);
      document.querySelector('.reader-pages')?.scrollTo(0, 0);
    } catch(e) { error = String(e); }
    finally { readerLoading = false; }
  }

  function goBack() {
    if (subView === 'reader') subView = 'detail';
    else if (subView === 'detail') { subView = 'browse'; detail = null; }
  }

  // Fallback de imágenes rotas (oculta el hueco)
  function onImgError(e) { e.currentTarget.style.visibility = 'hidden'; }
</script>

<!-- ══════════════════════════ BROWSE ══════════════════════════ -->
{#if subView === 'browse'}
<div class="view">
  <div class="header">
    {#if SOURCES.length > 1}
      <div class="source-tabs">
        {#each SOURCES as s}
          <button class="stab" class:active={activeSource === s.id} class:disabled={!s.ready}
            title={s.ready ? s.label : s.label + ' (próximamente)'}
            onclick={() => switchSource(s.id)}>
            {s.label}
            {#if !s.ready}<span class="soon-badge">pronto</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    <div class="search-wrap">
      <svg class="search-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input class="search-input" type="text" placeholder="Buscar manga..." bind:value={searchQuery} oninput={handleSearch} disabled={!source.ready} />
      {#if searchQuery}<button class="search-clear" onclick={clearSearch}>✕</button>{/if}
    </div>
  </div>

  {#if source.ready}
    <div class="browse-tabs">
      <button class="btab" class:active={activeTab === 'browse'} onclick={() => switchTab('browse')}>Populares</button>
      <button class="btab" class:active={activeTab === 'latest'} onclick={() => switchTab('latest')}>Recientes</button>
    </div>
  {/if}

  <div class="scroll-area">
    {#if !source.ready}
      <div class="status-center">
        <p class="empty-title">{source.label} — próximamente</p>
        <p class="empty-sub">Esta fuente aún no está disponible.</p>
      </div>
    {:else if loading}
      <div class="status-center"><div class="spinner"></div><span>Cargando...</span></div>
    {:else if error}
      <div class="status-center error">
        <span>{error}</span>
        <button class="retry-btn" onclick={() => switchTab(activeTab)}>↻ Reintentar</button>
      </div>
    {:else if items.length === 0}
      <div class="status-center">Sin resultados</div>
    {:else}
      {#if searchQuery}<div class="section-label">Resultados para "<strong>{searchQuery}</strong>"</div>{/if}
      <div class="cards-grid">
        {#each items as item (item.id)}
          <MangaCard {item} onclick={() => openManga(item)} latest={activeTab === 'latest'} />
        {/each}
      </div>
      {#if hasMore && !searchQuery}
        <div class="load-more-wrap">
          <button class="load-more-btn" onclick={loadMore} disabled={loadingMore}>
            {#if loadingMore}<div class="spinner-sm"></div> Cargando...{:else}Cargar más{/if}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<!-- ══════════════════════════ DETALLE ══════════════════════════ -->
{:else if subView === 'detail'}
<div class="view">
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>
    <span class="topbar-title">{detail?.title ?? ''}</span>
  </div>
  <div class="scroll-area">
    {#if loading && !detail?.chapters?.length}
      <div class="status-center"><div class="spinner"></div><span>Cargando info...</span></div>
    {:else if detail}
      <div class="detail-hero">
        <img class="detail-cover" src={detail.image} alt={detail.title} onerror={onImgError} />
        <div class="detail-info">
          <h1 class="detail-title">{detail.title}</h1>
          <div class="detail-meta">
            <span class="meta-badge">Manga</span>
            {#if detail.status}<span class="meta-badge status">{detail.status}</span>{/if}
          </div>
          {#if detail.authors?.length}<p class="detail-authors">{detail.authors.join(', ')}</p>{/if}
          {#if detail.genres?.length}
            <div class="detail-genres">{#each detail.genres.slice(0, 10) as g}<span class="genre-tag">{g}</span>{/each}</div>
          {/if}
          <button class="btn-library" class:in-library={inLib} onclick={() => toggleLibrary(detail)}>
            <svg viewBox="0 0 24 24" fill={inLib ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
            {inLib ? 'En tu biblioteca' : 'Añadir a biblioteca'}
          </button>
          {#if detail.description}<p class="detail-synopsis">{detail.description}</p>{/if}
        </div>
      </div>
      <div class="chapters-section">
        <div class="chapters-head">
          <h2 class="chapters-title">Capítulos {#if detail.chapters?.length}<span class="ch-count">{detail.chapters.length}</span>{/if}</h2>
          {#if detail.chapters?.length}
            <div class="head-actions">
              <button class="read-all-btn" class:all-read={allRead} onclick={markAllRead} title="Marcar todos como leídos">
                <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 10 8 14 16 6"/></svg>
                <span>{allRead ? 'Todo leído' : 'Leído todo'}</span>
              </button>
              <button class="dl-all-btn" class:dl-loading={downloadAllState === 'loading'} class:dl-done={downloadAllState === 'done'} class:dl-error={downloadAllState === 'error'} onclick={downloadAll} title="Descargar todos los capítulos">
                {#if downloadAllState === 'loading'}
                  <span class="dl-spinner"></span><span>{downloadAllProgress}/{detail.chapters.length}</span>
                {:else if downloadAllState === 'done'}
                  <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/></svg><span>Descargado</span>
                {:else if downloadAllState === 'error'}
                  <svg viewBox="0 0 20 20" fill="currentColor"><path d="M10 2a8 8 0 100 16A8 8 0 0010 2zm1 11H9v-2h2v2zm0-4H9V5h2v4z"/></svg><span>Reintentar</span>
                {:else}
                  <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg><span>Descargar todo</span>
                {/if}
              </button>
            </div>
          {/if}
        </div>

        {#if lastError}
          <div class="dl-error-bar"><span>⚠ {lastError}</span><button onclick={() => lastError = ''}>✕</button></div>
        {/if}

        {#if !detail.chapters?.length}
          <p class="no-chs">No se encontraron capítulos en español.</p>
        {:else}
          <div class="chapters-list">
            {#each detail.chapters as ch, i}
              <div class="ch-row" class:is-read={readSet.has(ch.id)} onclick={() => openChapter(i)} role="button" tabindex="0">
                <span class="ch-title">
                  {#if isDone(ch.id)}<span class="dl-tick">✓</span>{/if}
                  {ch.title}
                  {#if readSet.has(ch.id)}<span class="read-tag">Leído</span>{/if}
                </span>
                {#if ch.date}<span class="ch-date">{ch.date}</span>{/if}
                <button class="dl-btn" class:dl-loading={chapterState[ch.id] === 'loading'} class:dl-done={isDone(ch.id)} class:dl-error={chapterState[ch.id] === 'error'} onclick={(e) => downloadChapter(ch, e)} title="Descargar capítulo">
                  {#if chapterState[ch.id] === 'loading'}
                    <span class="dl-spinner"></span>
                  {:else if isDone(ch.id)}
                    <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
                  {:else if chapterState[ch.id] === 'error'}
                    <svg viewBox="0 0 20 20" fill="currentColor"><path d="M10 2a8 8 0 100 16A8 8 0 0010 2zm1 11H9v-2h2v2zm0-4H9V5h2v4z"/></svg>
                  {:else}
                    <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
                  {/if}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- ══════════════════════════ LECTOR ══════════════════════════ -->
{:else if subView === 'reader'}
<div class="view reader-view">
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>
    <span class="topbar-title">{currentChapter?.title ?? ''}</span>
    <div class="reader-nav">
      <button class="nav-btn" disabled={!prevChapter || readerLoading} onclick={() => openChapter(chapterIdx + 1)}>← Anterior</button>
      <button class="nav-btn" disabled={!nextChapter || readerLoading} onclick={() => openChapter(chapterIdx - 1)}>Siguiente →</button>
    </div>
  </div>
  <div class="reader-pages">
    {#if readerLoading}
      <div class="status-center"><div class="spinner large"></div><span>Cargando capítulo...</span></div>
    {:else if error}
      <div class="status-center error"><span>{error}</span></div>
    {:else if pages.length === 0}
      <div class="status-center">No se encontraron páginas.</div>
    {:else}
      {#each pages as p (p.url)}
        <img class="page-img" src={p.url} alt="" loading="lazy" onerror={onImgError} />
      {/each}
      <div class="reader-end">
        {#if nextChapter}
          <button class="end-btn" onclick={() => openChapter(chapterIdx - 1)}>Siguiente capítulo →</button>
        {:else}
          <span>Has llegado al último capítulo disponible.</span>
        {/if}
      </div>
    {/if}
  </div>
</div>
{/if}

<style>
  .view { display: flex; flex-direction: column; height: 100%; background: var(--bg); overflow: hidden; }

  /* Header */
  .header { display: flex; flex-direction: column; gap: 10px; padding: 14px 16px 10px; background: var(--bg-low); border-bottom: 1px solid var(--outline-dim); flex-shrink: 0; }
  .source-tabs { display: flex; gap: 6px; }
  .stab { display: flex; align-items: center; gap: 6px; padding: 6px 14px; border-radius: 8px; border: 1px solid var(--outline-dim); background: none; color: var(--text-muted); font-size: 12px; font-weight: 600; cursor: pointer; transition: background .15s, color .15s, border-color .15s; }
  .stab.active { background: var(--bg-card-high); color: var(--primary); border-color: var(--primary); }
  .stab.disabled { opacity: 0.5; }
  .soon-badge { font-size: 8px; font-weight: 800; background: rgba(255,255,255,0.08); color: var(--text-muted); padding: 1px 5px; border-radius: 4px; text-transform: uppercase; }

  .search-wrap { display: flex; align-items: center; gap: 8px; background: var(--bg-card); border: 1px solid var(--outline-dim); border-radius: 8px; padding: 0 10px; }
  .search-wrap:focus-within { border-color: var(--primary); }
  .search-ico { width: 14px; height: 14px; color: var(--text-muted); flex-shrink: 0; }
  .search-input { flex: 1; background: none; border: none; color: var(--text); font-size: 13px; padding: 8px 0; outline: none; }
  .search-input::placeholder { color: var(--text-muted); }
  .search-clear { background: none; border: none; color: var(--text-muted); cursor: pointer; font-size: 11px; padding: 2px 4px; }

  .browse-tabs { display: flex; gap: 2px; padding: 0 16px; border-bottom: 1px solid var(--outline-dim); flex-shrink: 0; }
  .btab { padding: 7px 14px; font-size: 12px; font-weight: 600; background: none; border: none; border-bottom: 2px solid transparent; color: var(--text-muted); cursor: pointer; margin-bottom: -1px; }
  .btab:hover { color: var(--text); }
  .btab.active { color: var(--primary); border-bottom-color: var(--primary); }

  .section-label { padding: 8px 16px 4px; font-size: 11px; font-weight: 700; color: var(--text-muted); text-transform: uppercase; letter-spacing: .05em; }
  .section-label strong { color: var(--text); }

  .scroll-area { flex: 1; overflow-y: auto; padding: 8px 12px 24px; }
  .cards-grid { display: flex; flex-wrap: wrap; gap: 12px; padding: 4px 0; }

  .status-center { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; min-height: 240px; color: var(--text-muted); font-size: 13px; text-align: center; }
  .status-center.error { color: var(--red, #ef4444); }
  .empty-title { font-size: 14px; font-weight: 700; color: var(--text); margin: 0; }
  .empty-sub { font-size: 12px; margin: 0; }
  .retry-btn { padding: 6px 16px; border-radius: 8px; border: 1px solid var(--outline-dim); background: var(--bg-card); color: var(--text); cursor: pointer; font-size: 12px; }

  .spinner, .spinner-sm, .spinner.large { border: 2px solid var(--outline-dim); border-top-color: var(--primary); border-radius: 50%; animation: spin .7s linear infinite; }
  .spinner { width: 26px; height: 26px; }
  .spinner.large { width: 38px; height: 38px; }
  .spinner-sm { width: 14px; height: 14px; display: inline-block; vertical-align: middle; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .load-more-wrap { display: flex; justify-content: center; padding: 16px 0 8px; }
  .load-more-btn { display: flex; align-items: center; gap: 7px; padding: 8px 24px; font-size: 12px; font-weight: 700; border-radius: 8px; border: 1px solid var(--outline-dim); background: var(--bg-card); color: var(--text-muted); cursor: pointer; }
  .load-more-btn:hover:not(:disabled) { background: var(--bg-card-high); color: var(--text); }
  .load-more-btn:disabled { opacity: .5; cursor: not-allowed; }

  /* Topbar (detalle/lector) */
  .topbar { display: flex; align-items: center; gap: 12px; padding: 8px 12px; background: var(--bg-low); border-bottom: 1px solid var(--outline-dim); flex-shrink: 0; }
  .back-btn { background: none; border: none; color: var(--primary); font-size: 13px; cursor: pointer; padding: 4px 8px; border-radius: 4px; white-space: nowrap; flex-shrink: 0; }
  .back-btn:hover { background: var(--bg-card); }
  .topbar-title { font-size: 12px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }

  /* Detalle */
  .detail-hero { display: flex; gap: 18px; padding: 16px 4px 20px; }
  .detail-cover { width: 150px; height: 214px; object-fit: cover; border-radius: 8px; flex-shrink: 0; background: var(--bg-card); }
  .detail-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 8px; }
  .detail-title { font-size: 18px; font-weight: 700; color: var(--text); line-height: 1.3; margin: 0; }
  .detail-meta { display: flex; gap: 6px; flex-wrap: wrap; }
  .meta-badge { font-size: 10px; font-weight: 700; padding: 2px 8px; border-radius: 5px; background: rgba(99,102,241,0.15); color: #818cf8; }
  .meta-badge.status { background: rgba(255,255,255,0.06); color: var(--text-muted); }
  .detail-authors { font-size: 12px; color: var(--text-muted); margin: 0; }
  .detail-genres { display: flex; flex-wrap: wrap; gap: 5px; }
  .genre-tag { font-size: 10px; background: rgba(255,255,255,0.06); color: var(--text-muted); padding: 2px 8px; border-radius: 4px; border: 1px solid var(--outline-dim); }
  .detail-synopsis { font-size: 12px; color: var(--text-muted); line-height: 1.6; margin: 0; display: -webkit-box; -webkit-line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; }

  /* Botón biblioteca */
  .btn-library { display: flex; align-items: center; gap: 6px; margin-top: 4px; padding: 6px 12px; border-radius: 7px; border: 1px solid var(--outline-dim); background: var(--bg-card); color: var(--text-muted); font-size: 11px; font-weight: 600; cursor: pointer; width: fit-content; transition: background .15s, color .15s, border-color .15s; }
  .btn-library svg { width: 14px; height: 14px; flex-shrink: 0; }
  .btn-library:hover { background: var(--bg-card-high); color: var(--text); border-color: var(--primary); }
  .btn-library.in-library { color: var(--primary); border-color: var(--primary); background: rgba(245,158,11,0.08); }

  .chapters-section { border-top: 1px solid var(--outline-dim); padding-top: 12px; }
  .chapters-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; flex-wrap: wrap; margin-bottom: 10px; }
  .head-actions { display: flex; align-items: center; gap: 6px; }
  .chapters-title { font-size: 13px; font-weight: 700; color: var(--text); margin: 0; display: flex; align-items: center; gap: 7px; }

  .read-all-btn { display: flex; align-items: center; gap: 4px; padding: 4px 10px; border-radius: 6px; border: 1px solid rgba(139,92,246,0.35); background: rgba(139,92,246,0.07); color: #a78bfa; font-size: 11px; font-weight: 600; cursor: pointer; }
  .read-all-btn svg { width: 12px; height: 12px; }
  .read-all-btn:hover { background: rgba(139,92,246,0.15); }
  .read-all-btn.all-read { color: #7c3aed; opacity: .6; cursor: default; }

  .dl-all-btn { display: flex; align-items: center; gap: 5px; padding: 4px 10px; border-radius: 6px; border: 1px solid rgba(34,197,94,0.4); background: rgba(34,197,94,0.08); color: #4ade80; font-size: 11px; font-weight: 600; cursor: pointer; }
  .dl-all-btn svg { width: 13px; height: 13px; flex-shrink: 0; }
  .dl-all-btn:hover { background: rgba(34,197,94,0.18); }
  .dl-all-btn.dl-done { background: rgba(34,197,94,0.15); }
  .dl-all-btn.dl-error { color: #f87171; border-color: rgba(239,68,68,0.35); background: rgba(239,68,68,0.08); }

  .dl-error-bar { display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; padding: 8px 12px; margin-bottom: 8px; background: rgba(239,68,68,0.08); border: 1px solid rgba(239,68,68,0.2); border-radius: 6px; font-size: 11px; color: #f87171; }
  .dl-error-bar button { background: none; border: none; color: #f87171; cursor: pointer; flex-shrink: 0; }

  .dl-tick { color: #4ade80; font-size: 12px; font-weight: 700; flex-shrink: 0; }
  .dl-btn { width: 24px; height: 24px; border-radius: 5px; border: 1px solid rgba(34,197,94,0.4); background: rgba(34,197,94,0.08); color: #4ade80; display: flex; align-items: center; justify-content: center; cursor: pointer; padding: 0; flex-shrink: 0; }
  .dl-btn svg { width: 13px; height: 13px; }
  .dl-btn:hover { background: rgba(34,197,94,0.18); }
  .dl-btn.dl-done { background: rgba(34,197,94,0.15); }
  .dl-btn.dl-error { color: #f87171; border-color: rgba(239,68,68,0.35); background: rgba(239,68,68,0.1); }
  .dl-spinner { width: 12px; height: 12px; border: 2px solid rgba(74,222,128,0.25); border-top-color: #4ade80; border-radius: 50%; animation: spin .7s linear infinite; flex-shrink: 0; }
  .ch-count { font-size: 10px; background: var(--bg-card); color: var(--text-muted); padding: 1px 7px; border-radius: 99px; font-weight: 600; }
  .no-chs { color: var(--text-muted); font-size: 12px; padding: 12px 0; }
  .chapters-list { display: flex; flex-direction: column; gap: 2px; }
  .ch-row { display: flex; align-items: center; gap: 10px; padding: 9px 12px; border-radius: 7px; cursor: pointer; border: 1px solid transparent; background: none; color: var(--text); text-align: left; width: 100%; font-size: 13px; }
  .ch-row:hover { background: var(--bg-card); border-color: var(--outline-dim); }
  .ch-row.is-read .ch-title { color: var(--text-muted); }
  .ch-title { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ch-date { font-size: 11px; color: var(--text-muted); flex-shrink: 0; }
  .read-tag { font-size: 9px; font-weight: 700; background: rgba(99,102,241,0.15); color: #818cf8; padding: 1px 5px; border-radius: 3px; flex-shrink: 0; }

  /* Lector */
  .reader-view { background: #0b0b0d; }
  .reader-nav { display: flex; gap: 6px; flex-shrink: 0; }
  .nav-btn { font-size: 11px; font-weight: 600; padding: 5px 10px; border-radius: 6px; border: 1px solid var(--outline-dim); background: var(--bg-card); color: var(--text); cursor: pointer; white-space: nowrap; }
  .nav-btn:disabled { opacity: .4; cursor: not-allowed; }
  .reader-pages { flex: 1; overflow-y: auto; display: flex; flex-direction: column; align-items: center; padding-bottom: 40px; }
  .page-img { width: 100%; max-width: 800px; height: auto; display: block; }
  .reader-end { display: flex; justify-content: center; padding: 24px; color: var(--text-muted); font-size: 13px; }
  .end-btn { padding: 10px 24px; border-radius: 8px; border: 1px solid var(--primary); background: var(--bg-card-high); color: var(--primary); cursor: pointer; font-weight: 700; font-size: 13px; }
</style>
