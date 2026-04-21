<script>
  import MangaCard from './MangaCard.svelte';
  import { tick, onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { invoke } from '@tauri-apps/api/core';
  import {
    currentTab, currentSource, latestItems, browseItems,
    latestHasMore, browseHasMore,
    loadingLatest, loadingBrowse, browseError,
    loadLatest, loadBrowse, openManga, openChapter, selectedManga, switchSource
  } from '../stores/manga.js';

  // Navega directamente al capítulo desde la card de Últimos.
  // Si tenemos chapter_id (URL) lo abrimos directo; si no, abrimos el detalle.
  async function handleOpenChapter(item, chapterId, chapterTitle) {
    selectedManga.set(item);
    if (chapterId) {
      openChapter({ id: chapterId, title: chapterTitle || '' }, item.id);
    } else {
      openManga(item);
    }
  }

  let latestSentinel = $state(null);
  let browseSentinel = $state(null);
  let obsLatest = null;
  let obsBrowse = null;
  let autoLoadingLatest = false;
  let autoLoadingBrowse = false;
  let refreshing = $state(false);

  // ── Búsqueda (pestaña Catálogo, todas las fuentes) ────────────────────────
  let searchQuery   = $state('');
  let searchResults = $state([]);
  let searchLoading = $state(false);
  let searchDebounce = null;

  // ── Filtros de género (Olympus, Cerberus y Taurus) ────────────────────────
  let availableGenres   = $state([]);
  let genresLoaded      = $state(false);   // false = aún no intentado/cargando
  let selectedGenres    = $state(new Set());
  let genreDropdownOpen = $state(false);
  let filterResults    = $state([]);
  let filterLoading    = $state(false);
  let filterHasMore    = $state(false);
  let filterPage       = $state(1);
  let filterIndexing   = $state(false); // solo Olympus — indexando ~20s

  let isBrowseTab = $derived($currentTab === 'browse');

  // Solo Olympus, Cerberus y Taurus tienen filtro de género
  let supportsGenres = $derived(
    isBrowseTab && ['olympus', 'cerberus', 'taurus'].includes($currentSource)
  );

  let isGenreActive = $derived(supportsGenres && selectedGenres.size > 0);

  // Search activa solo cuando no hay filtro de género
  let isSearchActive = $derived(
    isBrowseTab && searchQuery.trim().length > 0 && !isGenreActive
  );

  // Placeholder del input según fuente
  let searchPlaceholder = $derived(
    $currentSource === 'olympus'      ? 'Buscar en Olympus…'
    : $currentSource === 'cerberus'   ? 'Buscar en Cerberus Scans…'
    : $currentSource === 'taurus'     ? 'Buscar en Taurus Scan…'
    : 'Buscar en Leer Capitulo…'
  );

  // ── Búsqueda por texto ────────────────────────────────────────────────────
  function onSearchInput(e) {
    searchQuery = e.target.value;
    clearTimeout(searchDebounce);
    if (searchQuery.trim().length === 0) {
      searchResults = [];
      if (isGenreActive) { filterPage = 1; applyFilter(1); }
      return;
    }
    searchDebounce = setTimeout(() => {
      if (isGenreActive) { applyFilter(1); }
      else { doSearch(searchQuery.trim()); }
    }, 400);
  }

  async function doSearch(q) {
    searchLoading = true;
    try {
      const cmd =
        $currentSource === 'cerberus'     ? 'cerberus_search'
        : $currentSource === 'taurus'     ? 'taurus_search'
        : $currentSource === 'leercapitulo' ? 'leercapitulo_search'
        : 'olympus_search';
      const data = await invoke(cmd, { query: q });
      searchResults = data.results ?? [];
    } catch (e) {
      console.error('Error buscando:', e);
      searchResults = [];
    } finally {
      searchLoading = false;
    }
  }

  function clearSearch() {
    searchQuery = '';
    searchResults = [];
    clearTimeout(searchDebounce);
    if (isGenreActive) applyFilter(1);
  }

  // ── Filtros de género ─────────────────────────────────────────────────────
  async function openGenreDropdown() {
    genreDropdownOpen = !genreDropdownOpen;
    if (genreDropdownOpen && availableGenres.length === 0) {
      await loadGenres();
    }
  }

  async function loadGenres() {
    genresLoaded = false;
    try {
      const cmd =
        $currentSource === 'cerberus' ? 'cerberus_genres'
        : $currentSource === 'taurus' ? 'taurus_genres'
        : 'olympus_genres';
      availableGenres = await invoke(cmd);
    } catch (e) {
      console.error('Error cargando géneros:', e);
      availableGenres = [];
    } finally {
      genresLoaded = true;
    }
  }

  function toggleGenre(genre) {
    const next = new Set(selectedGenres);
    if (next.has(genre)) next.delete(genre); else next.add(genre);
    selectedGenres = next;
    filterPage = 1;
    filterResults = [];
    if (next.size > 0) applyFilter(1);
  }

  function clearGenres() {
    selectedGenres = new Set();
    filterResults = [];
    filterHasMore = false;
    filterIndexing = false;
    availableGenres = [];
    genresLoaded = false;
  }

  async function applyFilter(page = 1) {
    if (filterLoading) return;
    filterLoading = true;
    // Olympus requiere indexar ~800 series la primera vez; Cerberus/Taurus usan servidor
    if ($currentSource === 'olympus' && page === 1 && filterResults.length === 0) {
      filterIndexing = true;
    }
    try {
      const cmd =
        $currentSource === 'cerberus' ? 'cerberus_filter_genres'
        : $currentSource === 'taurus' ? 'taurus_filter_genres'
        : 'olympus_filter_genres';
      const data = await invoke(cmd, {
        genres: Array.from(selectedGenres),
        query: searchQuery.trim(),
        page,
      });
      const newItems = data.results ?? [];
      filterResults = page === 1 ? newItems : [...filterResults, ...newItems];
      filterHasMore = data.hasMore ?? false;
      filterPage = page;
    } catch (e) {
      console.error('Error filtrando:', e);
    } finally {
      filterLoading = false;
      filterIndexing = false;
    }
  }

  function handleOutsideClick(e) {
    if (genreDropdownOpen && !e.target.closest('.genre-wrap')) {
      genreDropdownOpen = false;
    }
  }

  // ── Infinite scroll ───────────────────────────────────────────────────────
  async function autoLoad(loadFn, hasMoreStore, loadingStore, getSentinel, flagRef, setFlag) {
    if (setFlag('get') || get(loadingStore) || !get(hasMoreStore)) return;
    const sentinel = getSentinel();
    if (!sentinel) return;
    const rect = sentinel.getBoundingClientRect();
    if (rect.top > window.innerHeight + 400) return;

    setFlag('set', true);
    try {
      await loadFn();
      await tick();
      const s2 = getSentinel();
      if (s2 && get(hasMoreStore)) {
        const r2 = s2.getBoundingClientRect();
        if (r2.top <= window.innerHeight + 400) {
          setTimeout(() => autoLoad(loadFn, hasMoreStore, loadingStore, getSentinel, flagRef, setFlag), 50);
        }
      }
    } finally {
      setFlag('set', false);
    }
  }

  function makeAutoLoadLatest() {
    return () => autoLoad(
      loadLatest, latestHasMore, loadingLatest, () => latestSentinel,
      null, (op, v) => { if (op === 'get') return autoLoadingLatest; autoLoadingLatest = v; }
    );
  }
  function makeAutoLoadBrowse() {
    return () => autoLoad(
      loadBrowse, browseHasMore, loadingBrowse, () => browseSentinel,
      null, (op, v) => { if (op === 'get') return autoLoadingBrowse; autoLoadingBrowse = v; }
    );
  }

  function makeObserver(triggerFn) {
    return new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting) triggerFn();
    }, { rootMargin: '400px' });
  }

  onMount(async () => {
    obsLatest = makeObserver(makeAutoLoadLatest());
    obsBrowse = makeObserver(makeAutoLoadBrowse());
    if ($latestItems.length === 0) await loadLatest(true);
    await tick();
    if (latestSentinel) obsLatest.observe(latestSentinel);
    makeAutoLoadLatest()();
  });

  onDestroy(() => { obsLatest?.disconnect(); obsBrowse?.disconnect(); });

  function onKeyDown(e) {
    if (e.key === 'F5') { e.preventDefault(); refresh(); }
  }

  $effect(() => { if (latestSentinel) obsLatest?.observe(latestSentinel); });
  $effect(() => { if (browseSentinel) obsBrowse?.observe(browseSentinel); });

  async function switchTab(tab) {
    if (tab !== 'browse') { clearSearch(); clearGenres(); }
    currentTab.set(tab);
    await tick();
    if (tab === 'latest') {
      if ($latestItems.length === 0) {
        await loadLatest(true); await tick(); makeAutoLoadLatest()();
      }
    } else {
      if ($browseItems.length === 0) {
        await loadBrowse(true); await tick(); makeAutoLoadBrowse()();
      }
    }
  }

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      if ($currentTab === 'latest') {
        await loadLatest(true); await tick(); makeAutoLoadLatest()();
      } else {
        if (isGenreActive) { filterResults = []; await applyFilter(1); }
        else { await loadBrowse(true); await tick(); makeAutoLoadBrowse()(); }
      }
    } finally { refreshing = false; }
  }

  async function changeSource(source) {
    clearSearch(); clearGenres();
    switchSource(source);
    await tick();
    if ($latestItems.length === 0) {
      await loadLatest(true); await tick(); makeAutoLoadLatest()();
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} onclick={handleOutsideClick} />

<div class="browse">
  <!-- Selector de fuente -->
  <div class="sources">
    {#each [
      { id: 'olympus',      label: 'Olympus' },
      { id: 'cerberus',     label: 'CerberusScan' },
      { id: 'taurus',       label: 'TaurusScan' },
      { id: 'leercapitulo', label: 'LeerCapitulo' },
    ] as src}
      <button
        class="source-btn"
        class:active={$currentSource === src.id}
        onclick={() => changeSource(src.id)}
      >{src.label}</button>
    {/each}
  </div>

  <!-- Pestañas Últimas / Catálogo -->
  <div class="tabs">
    <button class="tab" class:active={$currentTab === 'latest'} onclick={() => switchTab('latest')}>
      Últimas
    </button>
    <button class="tab" class:active={$currentTab === 'browse'} onclick={() => switchTab('browse')}>
      Catálogo
    </button>
    <button class="refresh-btn" class:spinning={refreshing} onclick={refresh} title="Refrescar" disabled={refreshing}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/>
        <path d="M21 3v5h-5"/>
        <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/>
        <path d="M3 21v-5h5"/>
      </svg>
    </button>
  </div>

  <!-- Búsqueda + Filtro de género — Catálogo (todas las fuentes) -->
  {#if isBrowseTab}
    <div class="search-bar">
      <!-- Input de búsqueda -->
      <div class="search-input-wrap">
        <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/>
        </svg>
        <input
          class="search-input"
          type="text"
          placeholder={searchPlaceholder}
          value={searchQuery}
          oninput={onSearchInput}
        />
        {#if searchQuery.length > 0}
          <button class="search-clear" onclick={clearSearch} title="Limpiar">✕</button>
        {/if}
      </div>

      <!-- Botón + dropdown de géneros — solo fuentes que lo soportan -->
      {#if supportsGenres}
      <div class="genre-wrap">
        <button
          class="genre-btn"
          class:active={selectedGenres.size > 0}
          onclick={openGenreDropdown}
          title="Filtrar por género"
        >
          <svg class="genre-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/>
          </svg>
          Géneros
          {#if selectedGenres.size > 0}
            <span class="genre-badge">{selectedGenres.size}</span>
          {/if}
          <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"
            style="transform: rotate({genreDropdownOpen ? 180 : 0}deg); transition: transform 0.2s">
            <polyline points="6 9 12 15 18 9"/>
          </svg>
        </button>

        {#if genreDropdownOpen}
          <div class="genre-dropdown">
            {#if selectedGenres.size > 0}
              <button class="genre-clear-all" onclick={() => { clearGenres(); genreDropdownOpen = false; }}>
                Limpiar filtros ({selectedGenres.size})
              </button>
            {/if}
            {#if !genresLoaded}
              <p class="genre-loading">Cargando géneros…</p>
            {:else if availableGenres.length === 0}
              <p class="genre-loading">No hay géneros disponibles</p>
            {:else}
              <div class="genre-list">
                {#each availableGenres as genre}
                  <label class="genre-option">
                    <input
                      type="checkbox"
                      checked={selectedGenres.has(genre)}
                      onchange={() => toggleGenre(genre)}
                    />
                    <span class="genre-name">{genre}</span>
                  </label>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
      {/if}
    </div>
  {/if}

  {#if $currentTab === 'latest'}
    <div class="grid-wrap">
      {#if $browseError && $latestItems.length === 0}
        <p class="error-msg">⚠ {$browseError}</p>
      {/if}
      <div class="grid">
        {#each $latestItems as item (item.id)}
          <MangaCard {item} onclick={openManga} latest={true}
            openchapter={(chId, chTitle) => handleOpenChapter(item, chId, chTitle)} />
        {/each}
      </div>
      <div bind:this={latestSentinel} class="sentinel"></div>
      {#if $loadingLatest}
        <p class="status-msg">Cargando...</p>
      {:else if !$latestHasMore && $latestItems.length > 0}
        <p class="status-msg dim">— Fin del listado —</p>
      {:else if $latestHasMore}
        <button class="load-more" onclick={() => loadLatest()}>Cargar más</button>
      {/if}
    </div>

  {:else}
    <div class="grid-wrap">
      {#if isGenreActive}
        <!-- ── Resultados de filtro de género ─────────────────────────────── -->
        {#if filterIndexing}
          <div class="indexing-notice">
            <span class="index-spinner"></span>
            <span>Indexando géneros por primera vez… puede tardar ~20 segundos</span>
          </div>
        {/if}

        {#if filterLoading && filterResults.length === 0 && !filterIndexing}
          <p class="status-msg">Filtrando…</p>
        {:else if filterResults.length === 0 && !filterLoading}
          <p class="status-msg dim">Sin resultados para los géneros seleccionados</p>
        {:else}
          {#if filterResults.length > 0}
            <p class="search-count">
              {filterResults.length}{filterHasMore ? '+' : ''} resultado{filterResults.length !== 1 ? 's' : ''}
              {#each selectedGenres as g}<span class="genre-tag">{g}</span>{/each}
            </p>
          {/if}
          <div class="grid">
            {#each filterResults as item (item.id)}
              <MangaCard {item} onclick={openManga} />
            {/each}
          </div>
          {#if filterLoading}
            <p class="status-msg">Cargando más…</p>
          {:else if filterHasMore}
            <button class="load-more" onclick={() => applyFilter(filterPage + 1)}>Cargar más</button>
          {:else if filterResults.length > 0}
            <p class="status-msg dim">— Fin de los resultados —</p>
          {/if}
        {/if}

      {:else if isSearchActive}
        <!-- ── Resultados de búsqueda por texto ──────────────────────────── -->
        {#if searchLoading}
          <p class="status-msg">Cargando catálogo completo, un momento…</p>
        {:else if searchResults.length === 0}
          <p class="status-msg dim">Sin resultados para «{searchQuery}»</p>
        {:else}
          <p class="search-count">{searchResults.length} resultado{searchResults.length !== 1 ? 's' : ''}</p>
          <div class="grid">
            {#each searchResults as item (item.id)}
              <MangaCard {item} onclick={openManga} />
            {/each}
          </div>
        {/if}

      {:else}
        <!-- ── Catálogo normal ─────────────────────────────────────────────── -->
        <div class="grid">
          {#each $browseItems as item (item.id)}
            <MangaCard {item} onclick={openManga} />
          {/each}
        </div>
        <div bind:this={browseSentinel} class="sentinel"></div>
        {#if $loadingBrowse}
          <p class="status-msg">Cargando...</p>
        {:else if !$browseHasMore && $browseItems.length > 0}
          <p class="status-msg dim">— Fin del catálogo —</p>
        {:else if $browseHasMore}
          <button class="load-more" onclick={() => loadBrowse()}>Cargar más</button>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .browse { display: flex; flex-direction: column; height: 100%; }

  /* Selector de fuente */
  .sources {
    display: flex;
    gap: 6px;
    padding: 8px 12px 6px;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .source-btn {
    background: var(--bg-card);
    border: 1px solid var(--outline-dim);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 20px;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }
  .source-btn:hover { color: var(--text); border-color: var(--outline); }
  .source-btn.active {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 10px 12px 0;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }
  .tab {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 13px;
    padding: 6px 14px;
    cursor: pointer;
    border-radius: 6px 6px 0 0;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--primary); border-bottom-color: var(--primary); }

  .refresh-btn {
    margin-left: auto;
    margin-bottom: 2px;
    background: none;
    border: 1px solid var(--outline-dim);
    border-radius: 6px;
    color: var(--text-muted);
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    transition: color 0.15s, border-color 0.15s, opacity 0.15s;
    flex-shrink: 0;
  }
  .refresh-btn svg { width: 14px; height: 14px; }
  .refresh-btn:hover { color: var(--primary); border-color: var(--primary); }
  .refresh-btn:disabled { opacity: 0.5; cursor: default; }
  .refresh-btn.spinning svg { animation: spin-btn 0.7s linear infinite; }
  @keyframes spin-btn { to { transform: rotate(360deg); } }

  /* ── Barra de búsqueda + filtros ─────────────────────────────────────────── */
  .search-bar {
    flex-shrink: 0;
    padding: 10px 12px 8px;
    border-bottom: 1px solid var(--outline-dim);
    background: var(--bg-card, #1a1f2e);
    display: flex;
    gap: 8px;
    align-items: center;
  }

  /* Input de búsqueda — ocupa la mitad del espacio */
  .search-input-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg, #111827);
    border: 1px solid var(--outline-dim);
    border-radius: 8px;
    padding: 7px 10px;
    transition: border-color 0.15s;
    min-width: 0;
  }
  .search-input-wrap:focus-within { border-color: var(--primary); }

  .search-icon { width: 15px; height: 15px; color: var(--text-muted); flex-shrink: 0; }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font-size: 13px;
    min-width: 0;
  }
  .search-input::placeholder { color: var(--text-muted); }

  .search-clear {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 12px;
    padding: 0 2px;
    line-height: 1;
    flex-shrink: 0;
    transition: color 0.15s;
  }
  .search-clear:hover { color: var(--text); }

  /* ── Filtro de géneros ───────────────────────────────────────────────────── */
  .genre-wrap {
    position: relative;
    flex-shrink: 0;
  }

  .genre-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: var(--bg, #111827);
    border: 1px solid var(--outline-dim);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    padding: 7px 11px;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
    height: 100%;
  }
  .genre-btn:hover { color: var(--text); border-color: var(--outline); }
  .genre-btn.active {
    color: var(--primary);
    border-color: var(--primary);
    background: color-mix(in srgb, var(--primary) 10%, transparent);
  }

  .genre-icon { width: 13px; height: 13px; flex-shrink: 0; }
  .chevron    { width: 13px; height: 13px; flex-shrink: 0; }

  .genre-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--primary);
    color: var(--on-primary);
    font-size: 10px;
    font-weight: 700;
    min-width: 16px;
    height: 16px;
    border-radius: 8px;
    padding: 0 4px;
  }

  .genre-dropdown {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 200;
    background: var(--bg-card, #1a1f2e);
    border: 1px solid var(--outline);
    border-radius: 10px;
    min-width: 180px;
    max-width: 220px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.5);
    overflow: hidden;
  }

  .genre-clear-all {
    display: block;
    width: 100%;
    text-align: left;
    padding: 8px 12px;
    background: none;
    border: none;
    border-bottom: 1px solid var(--outline-dim);
    color: var(--primary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s;
  }
  .genre-clear-all:hover { background: color-mix(in srgb, var(--primary) 8%, transparent); }

  .genre-loading {
    padding: 12px;
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
  }

  .genre-list {
    max-height: 280px;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
    scrollbar-color: var(--outline-dim) transparent;
  }

  .genre-option {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    cursor: pointer;
    transition: background 0.1s;
  }
  .genre-option:hover { background: var(--bg, #111827); }

  .genre-option input[type="checkbox"] {
    width: 14px;
    height: 14px;
    accent-color: var(--primary);
    cursor: pointer;
    flex-shrink: 0;
  }

  .genre-name {
    font-size: 12px;
    color: var(--text);
    line-height: 1.3;
  }

  /* ── Aviso de indexado ───────────────────────────────────────────────────── */
  .indexing-notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    margin-bottom: 12px;
    background: color-mix(in srgb, var(--primary) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--primary) 25%, transparent);
    border-radius: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .index-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--outline-dim);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin-idx 0.8s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin-idx { to { transform: rotate(360deg); } }

  /* ── Tags de género activo ───────────────────────────────────────────────── */
  .genre-tag {
    display: inline-block;
    background: color-mix(in srgb, var(--primary) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--primary) 30%, transparent);
    color: var(--primary);
    font-size: 10px;
    font-weight: 600;
    padding: 1px 7px;
    border-radius: 10px;
    margin-left: 5px;
    vertical-align: middle;
  }

  /* ── Grid y paginación ───────────────────────────────────────────────────── */
  .grid-wrap {
    flex: 1;
    overflow-y: auto;
    padding: 14px 12px;
    min-height: 0;
  }

  .grid { display: flex; flex-wrap: wrap; gap: 12px; }

  .sentinel { width: 100%; height: 20px; }

  .status-msg {
    text-align: center;
    color: var(--text-muted);
    padding: 16px;
    font-size: 13px;
  }
  .status-msg.dim { color: var(--outline); font-size: 12px; }

  .error-msg {
    color: var(--red, #f87171);
    font-size: 12px;
    padding: 12px 16px;
    background: rgba(248,113,113,0.08);
    border: 1px solid rgba(248,113,113,0.2);
    border-radius: 6px;
    margin-bottom: 12px;
    word-break: break-all;
  }

  .load-more {
    display: block;
    margin: 12px auto;
    padding: 8px 24px;
    background: var(--bg-card);
    border: 1px solid var(--outline);
    color: var(--text-muted);
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: border-color 0.15s, color 0.15s;
  }
  .load-more:hover { border-color: var(--primary); color: var(--primary); }

  .search-count {
    font-size: 11px;
    color: var(--text-muted);
    padding: 0 0 10px;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
  }
</style>
