<script>
  import { currentManga, loadingDetail, selectedManga, openChapter, goBack, library, toggleLibrary, downloadPath, autoDownloadConfig, toggleAutoDownload, anilistMap, setAniListId } from '../stores/manga.js';
  import { invoke } from '@tauri-apps/api/core';

  let synopsisExpanded = $state(false);

  // ── Descarga ──────────────────────────────────────────────────────────────
  // Estado de sesión: 'loading' | 'error' (en tránsito)
  let chapterState    = $state({});
  let downloadAllState    = $state('idle');
  let downloadAllProgress = $state(0);
  let lastError = $state('');

  // Capítulos ya descargados (persistidos en localStorage)
  let downloadedSet = $state(new Set());
  // Capítulos leídos (persistidos en localStorage)
  let readSet = $state(new Set());

  // ── Persistencia localStorage ──────────────────────────────────────────
  function loadDownloaded(mangaId) {
    try {
      const raw = localStorage.getItem(`foundry_dl_${mangaId}`);
      return raw ? new Set(JSON.parse(raw)) : new Set();
    } catch { return new Set(); }
  }

  function saveDownloaded(mangaId, ids) {
    try {
      localStorage.setItem(`foundry_dl_${mangaId}`, JSON.stringify([...ids]));
    } catch {}
  }

  function loadRead(mangaId) {
    try {
      const raw = localStorage.getItem(`foundry_read_${mangaId}`);
      return raw ? new Set(JSON.parse(raw)) : new Set();
    } catch { return new Set(); }
  }

  function saveRead(mangaId, ids) {
    try {
      localStorage.setItem(`foundry_read_${mangaId}`, JSON.stringify([...ids]));
    } catch {}
  }

  // Al cambiar de manga: carga persistidos y resetea estado de sesión
  $effect(() => {
    const id = $currentManga?.id;
    downloadedSet       = id ? loadDownloaded(id) : new Set();
    readSet             = id ? loadRead(id) : new Set();
    chapterState        = {};
    downloadAllState    = 'idle';
    downloadAllProgress = 0;
  });

  function markRead(chapterId) {
    const mangaId = $currentManga?.id;
    if (!mangaId) return;
    readSet = new Set([...readSet, chapterId]);
    saveRead(mangaId, readSet);
  }

  function markAllRead() {
    const mangaId  = $currentManga?.id;
    const chapters = $currentManga?.chapters ?? [];
    if (!mangaId) return;
    readSet = new Set(chapters.map(c => c.id));
    saveRead(mangaId, readSet);
  }

  function handleOpenChapter(chapter) {
    markRead(chapter.id);
    openChapter(chapter, $currentManga.id);
  }

  let allRead = $derived(
    ($currentManga?.chapters?.length ?? 0) > 0 &&
    ($currentManga?.chapters ?? []).every(c => readSet.has(c.id))
  );

  // ¿Este capítulo ya está descargado (sesión o persistido)?
  function isDone(chapterId) {
    return chapterState[chapterId] === 'done' || downloadedSet.has(chapterId);
  }

  async function downloadChapter(chapter, e) {
    e.stopPropagation();
    if (chapterState[chapter.id] === 'loading' || isDone(chapter.id)) return;

    const mangaId    = $currentManga.id;
    const mangaTitle = $currentManga.title;
    chapterState = { ...chapterState, [chapter.id]: 'loading' };
    try {
      await invoke('download_chapter', {
        mangaId,
        mangaTitle,
        chapterId:    chapter.id,
        chapterTitle: chapter.title,
        downloadPath: $downloadPath || null,
      });
      chapterState = { ...chapterState, [chapter.id]: 'done' };
      downloadedSet = new Set([...downloadedSet, chapter.id]);
      saveDownloaded(mangaId, downloadedSet);
    } catch (err) {
      const msg = typeof err === 'string' ? err : (err?.message ?? String(err));
      console.error('Error descargando capítulo:', msg);
      lastError = msg;
      chapterState = { ...chapterState, [chapter.id]: 'error' };
    }
  }

  async function downloadAll(e) {
    e.stopPropagation();
    if (downloadAllState === 'loading') return;
    const chapters   = $currentManga?.chapters ?? [];
    const mangaId    = $currentManga?.id;
    const mangaTitle = $currentManga?.title;
    if (!chapters.length || !mangaId) return;

    downloadAllState    = 'loading';
    downloadAllProgress = 0;

    for (const chapter of chapters) {
      if (isDone(chapter.id)) {
        downloadAllProgress++;
        continue;
      }
      chapterState = { ...chapterState, [chapter.id]: 'loading' };
      try {
        await invoke('download_chapter', {
          mangaId,
          mangaTitle,
          chapterId:    chapter.id,
          chapterTitle: chapter.title,
          downloadPath: $downloadPath || null,
        });
        chapterState    = { ...chapterState, [chapter.id]: 'done' };
        downloadedSet   = new Set([...downloadedSet, chapter.id]);
        saveDownloaded(mangaId, downloadedSet);
      } catch {
        chapterState = { ...chapterState, [chapter.id]: 'error' };
      }
      downloadAllProgress++;
    }

    downloadAllState = downloadAllProgress === chapters.length ? 'done' : 'error';
  }

  // ── AniList ────────────────────────────────────────────────────────────────
  let anilistSearching = $state(false);
  let anilistMatches   = $state([]);
  let anilistShowPick  = $state(false);
  let anilistSyncing   = $state(false);

  async function openAniListPicker() {
    if (!$currentManga) return;
    anilistShowPick  = !anilistShowPick;
    if (!anilistShowPick || anilistMatches.length > 0) return;
    anilistSearching = true;
    try {
      anilistMatches = await invoke('anilist_search', { title: $currentManga.title });
    } catch { anilistMatches = []; }
    finally { anilistSearching = false; }
  }

  function pickAniList(match) {
    setAniListId($currentManga.id, match.id);
    anilistShowPick = false;
    anilistMatches  = [];
  }

  async function syncAniList() {
    const manga   = $currentManga;
    const aniId   = $anilistMap[manga?.id];
    if (!manga || !aniId) return;
    anilistSyncing = true;
    // Usa el índice del capítulo más reciente leído (readSet)
    const chapters = manga.chapters ?? [];
    const readCount = chapters.filter(c => readSet.has(c.id)).length;
    try {
      await invoke('anilist_update_progress', { mediaId: aniId, progress: readCount });
    } catch (e) { console.error('AniList sync error:', e); }
    finally { anilistSyncing = false; }
  }

  function statusColor(status) {
    const s = (status || '').toLowerCase();
    if (s.includes('activo'))     return 'var(--green)';
    if (s.includes('pausado'))    return 'var(--orange)';
    if (s.includes('cancelado'))  return 'var(--red)';
    return 'var(--gray)';
  }

  function formatDate(dateStr) {
    if (!dateStr) return '';
    const d = new Date(dateStr);
    return isNaN(d) ? dateStr : d.toLocaleDateString('es-ES');
  }
</script>

<div class="detail">
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>
  </div>

  {#if $loadingDetail}
    <div class="loading-state">
      <div class="skeleton cover-skel"></div>
      <p class="loading-text">Cargando...</p>
    </div>

  {:else if $currentManga}
    <div class="scroll-area">
      <!-- ── Hero banner ─────────────────────────────────────────────────── -->
      <div class="hero">
        <img
          class="cover"
          src={$currentManga.image}
          alt={$currentManga.title}
          onerror={(e) => e.target.src='https://picsum.photos/600/800'}
        />
        <div class="meta">
          <h2 class="manga-title">{$currentManga.title}</h2>

          {#if $currentManga.status}
            <div class="status-badge" style="color: {statusColor($currentManga.status)}">
              <span class="dot" style="background: {statusColor($currentManga.status)}"></span>
              {$currentManga.status}
            </div>
          {/if}

          {#if $currentManga.authors?.length}
            <p class="author">{$currentManga.authors.join(', ')}</p>
          {/if}

          {#if $currentManga.genres?.length}
            <div class="genres">
              {#each $currentManga.genres as genre}
                <span class="genre-tag">{genre}</span>
              {/each}
            </div>
          {/if}

          <!-- Botón biblioteca + controles extra -->
          {#if $currentManga}
            {@const inLib    = $library.some(e => e.id === $currentManga.id)}
            {@const aniId    = $anilistMap[$currentManga.id]}
            {@const autoDl   = $autoDownloadConfig[$currentManga.id]}
            <div class="detail-actions">
              <button
                class="btn-library"
                class:in-library={inLib}
                onclick={() => toggleLibrary($currentManga)}
              >
                <svg viewBox="0 0 24 24" fill={inLib ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
                </svg>
                {inLib ? 'En tu biblioteca' : 'Añadir a biblioteca'}
              </button>

              {#if inLib}
                <!-- Auto-descarga toggle -->
                <button
                  class="btn-mini"
                  class:btn-mini-on={autoDl}
                  onclick={() => toggleAutoDownload($currentManga.id)}
                  title={autoDl ? 'Auto-descarga activada' : 'Activar auto-descarga'}
                >
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                    <polyline points="7 10 12 15 17 10"/>
                    <line x1="12" y1="15" x2="12" y2="3"/>
                  </svg>
                  {autoDl ? 'Auto ✓' : 'Auto-dl'}
                </button>

                <!-- AniList link/sync -->
                <div class="anilist-wrap">
                  <button
                    class="btn-mini"
                    class:btn-mini-on={!!aniId}
                    onclick={openAniListPicker}
                    title={aniId ? `AniList ID: ${aniId}` : 'Vincular con AniList'}
                  >
                    AL {aniId ? '✓' : '+'}
                  </button>
                  {#if aniId}
                    <button
                      class="btn-mini"
                      class:btn-mini-loading={anilistSyncing}
                      onclick={syncAniList}
                      title="Sincronizar progreso con AniList"
                      disabled={anilistSyncing}
                    >
                      {anilistSyncing ? '…' : '↑ Sync'}
                    </button>
                  {/if}

                  {#if anilistShowPick}
                    <div class="anilist-picker">
                      {#if anilistSearching}
                        <p class="al-loading">Buscando en AniList…</p>
                      {:else if anilistMatches.length === 0}
                        <p class="al-loading">Sin resultados</p>
                      {:else}
                        {#each anilistMatches as m}
                          <button class="al-match" onclick={() => pickAniList(m)}>
                            {#if m.cover}<img src={m.cover} alt="" class="al-thumb" />{/if}
                            <span class="al-title">{m.title}</span>
                            <span class="al-prog">{m.progress} leídos</span>
                          </button>
                        {/each}
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>

      <!-- ── Sinopsis colapsable ────────────────────────────────────────── -->
      {#if $currentManga.description}
        <button class="strip synopsis-strip" onclick={() => synopsisExpanded = !synopsisExpanded}>
          <span>Sinopsis</span>
          <span class="arrow">{synopsisExpanded ? '▾' : '▸'}</span>
        </button>
        {#if synopsisExpanded}
          <div class="synopsis-text">
            <p>{$currentManga.description}</p>
          </div>
        {/if}
      {/if}

      <!-- ── Strip de capítulos con botón descargar todos ──────────────── -->
      <div class="strip chapters-strip">
        <span>{$currentManga.chapters?.length || 0} capítulos</span>

        <div class="strip-actions">
          <button
            class="read-all-btn"
            class:all-read={allRead}
            onclick={markAllRead}
            title="Marcar todos como leídos"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="4 10 8 14 16 6"/>
            </svg>
            <span>{allRead ? 'Todo leído' : 'Leído todo'}</span>
          </button>

        <button
            class="dl-all-btn"
            class:dl-loading={downloadAllState === 'loading'}
            class:dl-done={downloadAllState === 'done'}
            class:dl-error={downloadAllState === 'error'}
            onclick={downloadAll}
            title="Descargar todos los capítulos"
          >
            {#if downloadAllState === 'loading'}
              <span class="dl-spinner"></span>
              <span class="dl-all-label">
                {downloadAllProgress}/{$currentManga.chapters?.length ?? 0}
              </span>
            {:else if downloadAllState === 'done'}
              <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
              <span class="dl-all-label">Descargado</span>
            {:else if downloadAllState === 'error'}
              <svg viewBox="0 0 20 20" fill="currentColor"><path d="M10 2a8 8 0 100 16A8 8 0 0010 2zm1 11H9v-2h2v2zm0-4H9V5h2v4z"/></svg>
              <span class="dl-all-label">Reintentar</span>
            {:else}
              <svg viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/>
              </svg>
              <span class="dl-all-label">Descargar todo</span>
            {/if}
          </button>
        </div>
      </div>

      <!-- ── Error de descarga ─────────────────────────────────────────── -->
      {#if lastError}
        <div class="dl-error-bar">
          <span>⚠ {lastError}</span>
          <button onclick={() => lastError = ''}>✕</button>
        </div>
      {/if}

      <!-- ── Lista de capítulos ─────────────────────────────────────────── -->
      <div class="chapters">
        {#each ($currentManga.chapters || []) as chapter (chapter.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="chapter-item" class:is-read={readSet.has(chapter.id)} onclick={() => handleOpenChapter(chapter)}>
            <span class="chapter-title">
              {#if isDone(chapter.id)}
                <span class="dl-tick">✓</span>
              {/if}
              {chapter.title}
              {#if readSet.has(chapter.id)}
                <span class="read-tag">Leído</span>
              {/if}
            </span>
            <div class="chapter-right">
              <span class="chapter-date">{formatDate(chapter.date)}</span>

              <button
                  class="dl-btn"
                  class:dl-loading={chapterState[chapter.id] === 'loading'}
                  class:dl-done={isDone(chapter.id)}
                  class:dl-error={chapterState[chapter.id] === 'error'}
                  onclick={(e) => downloadChapter(chapter, e)}
                  title="Descargar capítulo"
                >
                  {#if chapterState[chapter.id] === 'loading'}
                    <span class="dl-spinner"></span>
                  {:else if isDone(chapter.id)}
                    <!-- Checkmark -->
                    <svg viewBox="0 0 20 20" fill="currentColor">
                      <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/>
                    </svg>
                  {:else if chapterState[chapter.id] === 'error'}
                    <!-- Exclamación -->
                    <svg viewBox="0 0 20 20" fill="currentColor">
                      <path d="M10 2a8 8 0 100 16A8 8 0 0010 2zm1 11H9v-2h2v2zm0-4H9V5h2v4z"/>
                    </svg>
                  {:else}
                    <!-- Flecha descargar -->
                    <svg viewBox="0 0 20 20" fill="currentColor">
                      <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/>
                    </svg>
                  {/if}
                </button>
            </div>
          </div>
        {/each}
      </div>
    </div>

  {:else}
    <p class="error-text">No se pudo cargar la información.</p>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
  }

  .topbar {
    display: flex;
    align-items: center;
    padding: 8px 12px;
    background: var(--bg-low);
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .back-btn {
    background: none;
    border: none;
    color: var(--primary);
    font-size: 13px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
  }
  .back-btn:hover { background: var(--bg-card); }

  .scroll-area { flex: 1; overflow-y: auto; }

  .hero {
    display: flex;
    gap: 12px;
    padding: 14px 12px;
    background: var(--bg-low);
  }

  .cover {
    width: 80px;
    height: 112px;
    object-fit: cover;
    border-radius: 6px;
    flex-shrink: 0;
  }

  .meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .manga-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text);
    line-height: 1.3;
    word-break: break-word;
  }

  .status-badge {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    font-weight: 600;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .author { font-size: 11px; color: var(--text-muted); }

  .genres { display: flex; flex-wrap: wrap; gap: 4px; }
  .genre-tag {
    font-size: 10px;
    padding: 2px 7px;
    border-radius: 9px;
    background: var(--bg-card);
    border: 1px solid var(--secondary);
    color: var(--secondary);
  }

  .btn-library {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 6px 12px;
    border-radius: 7px;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    width: fit-content;
  }
  .btn-library svg { width: 14px; height: 14px; flex-shrink: 0; }
  .btn-library:hover { background: var(--bg-card-high); color: var(--text); border-color: var(--primary); }
  .btn-library.in-library { color: var(--primary); border-color: var(--primary); background: rgba(245,158,11,0.08); }

  .detail-actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin-top: 8px; }

  .btn-mini {
    display: flex; align-items: center; gap: 4px;
    padding: 5px 10px; border-radius: 7px;
    border: 1px solid var(--outline-dim); background: var(--bg-card);
    color: var(--text-muted); font-size: 10px; font-weight: 700;
    cursor: pointer; white-space: nowrap;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .btn-mini:hover { color: var(--text); border-color: var(--outline); }
  .btn-mini.btn-mini-on { color: var(--green, #4ade80); border-color: var(--green, #4ade80); background: rgba(74,222,128,0.08); }
  .btn-mini.btn-mini-loading { opacity: 0.6; cursor: default; }

  .anilist-wrap { position: relative; display: flex; gap: 6px; align-items: center; }

  .anilist-picker {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 300;
    background: var(--bg-card); border: 1px solid var(--outline);
    border-radius: 10px; min-width: 220px; max-width: 280px;
    max-height: 260px; overflow-y: auto;
    box-shadow: 0 8px 24px rgba(0,0,0,0.5);
    padding: 6px 0;
  }
  .al-loading { padding: 12px; font-size: 12px; color: var(--text-muted); text-align: center; }
  .al-match {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 6px 10px; border: none; background: none;
    cursor: pointer; text-align: left; transition: background 0.1s;
  }
  .al-match:hover { background: var(--bg); }
  .al-thumb { width: 32px; height: 44px; object-fit: cover; border-radius: 4px; flex-shrink: 0; }
  .al-title { flex: 1; font-size: 11px; color: var(--text); line-height: 1.3; }
  .al-prog { font-size: 10px; color: var(--text-muted); white-space: nowrap; }

  .strip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    height: 36px;
    background: var(--bg-card);
    border-top: 1px solid var(--outline-dim);
    border-bottom: 1px solid var(--outline-dim);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
    width: 100%;
  }

  .synopsis-strip { cursor: pointer; border: none; text-align: left; }
  .synopsis-strip:hover { background: var(--bg-card-high); }
  .arrow { color: var(--primary); }

  .synopsis-text {
    padding: 12px;
    background: var(--bg-low);
    border-bottom: 1px solid var(--outline-dim);
  }
  .synopsis-text p {
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.6;
    white-space: pre-line;
  }

  .chapters-strip { cursor: default; }

  .strip-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Botón Leído todo */
  .read-all-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(139,92,246,0.35);
    background: rgba(139,92,246,0.07);
    color: #a78bfa;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }
  .read-all-btn svg { width: 12px; height: 12px; flex-shrink: 0; }
  .read-all-btn:hover { background: rgba(139,92,246,0.15); border-color: rgba(139,92,246,0.6); }
  .read-all-btn.all-read { color: #7c3aed; opacity: 0.6; cursor: default; }

  /* ── Lista de capítulos ── */
  .chapters { display: flex; flex-direction: column; }

  .chapter-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    border-bottom: 1px solid var(--outline-dim);
    cursor: pointer;
    transition: background 0.1s;
  }
  .chapter-item:hover { background: var(--bg-card); }

  .chapter-item.is-read .chapter-title { color: var(--text-muted); }

  .chapter-title { font-size: 13px; color: var(--text); flex: 1; min-width: 0; display: flex; align-items: center; gap: 5px; }
  .dl-tick { color: #4ade80; font-size: 12px; font-weight: 700; flex-shrink: 0; }

  .read-tag {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: #7c3aed;
    background: rgba(139,92,246,0.12);
    border: 1px solid rgba(139,92,246,0.25);
    border-radius: 4px;
    padding: 1px 5px;
    flex-shrink: 0;
    text-transform: uppercase;
  }

  .chapter-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .chapter-date { font-size: 11px; color: var(--text-muted); }

  /* ── Botón de descarga por capítulo ── */
  .dl-btn {
    width: 24px;
    height: 24px;
    border-radius: 5px;
    border: 1px solid rgba(34,197,94,0.4);
    background: rgba(34,197,94,0.08);
    color: #4ade80;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }
  .dl-btn svg { width: 13px; height: 13px; }
  .dl-btn:hover { background: rgba(34,197,94,0.18); border-color: rgba(34,197,94,0.7); }

  .dl-btn.dl-done  { color: #4ade80; background: rgba(34,197,94,0.15); border-color: rgba(34,197,94,0.5); }
  .dl-btn.dl-error { color: #f87171; background: rgba(239,68,68,0.1);  border-color: rgba(239,68,68,0.35); }
  .dl-btn.dl-loading { color: #4ade80; cursor: default; }

  /* ── Botón descargar todos ── */
  .dl-all-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(34,197,94,0.4);
    background: rgba(34,197,94,0.08);
    color: #4ade80;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }
  .dl-all-btn svg { width: 13px; height: 13px; flex-shrink: 0; }
  .dl-all-btn:hover { background: rgba(34,197,94,0.18); border-color: rgba(34,197,94,0.7); }

  .dl-all-btn.dl-done  { color: #4ade80; background: rgba(34,197,94,0.15); }
  .dl-all-btn.dl-error { color: #f87171; border-color: rgba(239,68,68,0.35); background: rgba(239,68,68,0.08); }
  .dl-all-btn.dl-loading { cursor: default; }

  .dl-all-label { white-space: nowrap; }

  /* Spinner común */
  .dl-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(74,222,128,0.25);
    border-top-color: #4ade80;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Estados de carga y error */
  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 40px 20px;
    gap: 16px;
  }
  .skeleton { background: var(--bg-card); border-radius: 6px; animation: pulse 1.2s infinite; }
  .cover-skel { width: 80px; height: 112px; }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
  .loading-text, .error-text { color: var(--text-muted); font-size: 13px; }
  .error-text { padding: 20px; }

  .dl-error-bar {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px;
    background: rgba(239,68,68,0.08);
    border-bottom: 1px solid rgba(239,68,68,0.2);
    font-size: 11px;
    color: #f87171;
    word-break: break-word;
  }
  .dl-error-bar button {
    background: none;
    border: none;
    color: #f87171;
    cursor: pointer;
    font-size: 12px;
    flex-shrink: 0;
    padding: 0 2px;
  }
</style>
