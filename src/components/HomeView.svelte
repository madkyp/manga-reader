<script context="module">
  let cachedOlympus  = [];
  let cachedCerberus = [];
  let cachedTaurus   = [];
  let cachedLeer     = [];
  let cachedAnimeFlv = [];
  let cacheLoaded    = false;

  function stampDates(arr) {
    const now = new Date().toISOString();
    let tsCache;
    try { tsCache = JSON.parse(localStorage.getItem('foundry_latest_ts') || '{}'); }
    catch { tsCache = {}; }
    let changed = false;
    const out = (arr ?? []).map(item => {
      if (item.date) return item;
      if (!tsCache[item.id]) { tsCache[item.id] = now; changed = true; }
      return { ...item, date: tsCache[item.id] };
    });
    if (changed) { try { localStorage.setItem('foundry_latest_ts', JSON.stringify(tsCache)); } catch {} }
    return out;
  }
</script>

<script>
  import { onMount, onDestroy } from 'svelte';
  import { latestItems, browseItems, openManga, openChapter, selectedManga, currentManga, readHistory, readStats, titleCache, currentView } from '../stores/manga.js';
  import { pendingAnime } from '../stores/anime.js';
  import { invoke } from '@tauri-apps/api/core';

  // ── Banner rotatorio ──────────────────────────────────────────────────────
  let featuredIndex = $state(0);
  let featuredDetail = $state(null);
  let loadingDetail  = $state(false);
  let rotateTimer    = null;
  let refreshTimer   = null;
  let lastRefresh    = $state(null);   // timestamp del último refresco
  let refreshing     = $state(false);  // indica refresco silencioso en curso

  let pool = $derived.by(() => {
    const seen = new Set();
    const out = [];
    for (const item of [...updatesOlympus, ...$latestItems, ...$browseItems]) {
      if (!seen.has(item.id)) { seen.add(item.id); out.push(item); if (out.length === 20) break; }
    }
    return out;
  });

  let featured = $derived(pool[featuredIndex] ?? null);

  onMount(async () => {
    startRotation();
    loadUpdates();
    // Auto-refresco cada 5 min, sincronizado con el check de notificaciones
    refreshTimer = setInterval(() => silentRefresh(), 5 * 60_000);
  });

  onDestroy(() => {
    clearInterval(rotateTimer);
    clearInterval(refreshTimer);
  });

  // Refresco silencioso: no muestra spinner, solo actualiza si hay datos nuevos
  async function silentRefresh() {
    if (refreshing || loadingUpdates) return;
    refreshing = true;
    try {
      const data = await invoke('get_home_updates');

      const newOly  = stampDates(data.olympus      ?? []);
      const newCer  = stampDates(data.cerberus      ?? []);
      const newTau  = stampDates(data.taurus        ?? []);
      const newLeer = stampDates(data.leercapitulo  ?? []);
      const newFlv  = data.animeflv                 ?? [];

      if (newOly[0]?.id  !== cachedOlympus[0]?.id)  { cachedOlympus  = newOly;  updatesOlympus  = newOly; }
      if (newCer[0]?.id  !== cachedCerberus[0]?.id) { cachedCerberus = newCer;  updatesCerberus = newCer; }
      if (newTau[0]?.id  !== cachedTaurus[0]?.id)   { cachedTaurus   = newTau;  updatesTaurus   = newTau; }
      if (newLeer[0]?.id !== cachedLeer[0]?.id)     { cachedLeer     = newLeer; updatesLeer     = newLeer; }
      if (newFlv[0]?.id  !== cachedAnimeFlv[0]?.id) { cachedAnimeFlv = newFlv;  updatesAnimeFlv = newFlv; }

      cacheLoaded = true;
      lastRefresh = Date.now();

      titleCache.update(c => {
        [...newOly, ...newCer, ...newTau, ...newLeer]
          .forEach(item => { if (item.id && item.title) c[item.id] = item.title; });
        return c;
      });
    } catch {}
    finally {
      refreshing = false;
    }
  }

  function startRotation() {
    clearInterval(rotateTimer);
    rotateTimer = setInterval(() => {
      if (pool.length > 0) featuredIndex = (featuredIndex + 1) % pool.length;
    }, 8000);
  }

  $effect(() => {
    if (!featured) return;
    featuredDetail = null;
    loadFeaturedDetail(featured.id);
  });

  async function loadFeaturedDetail(id) {
    loadingDetail = true;
    try {
      featuredDetail = await invoke('get_info', { id });
    } catch {
      featuredDetail = null;
    } finally {
      loadingDetail = false;
    }
  }

  function prev() { featuredIndex = (featuredIndex - 1 + pool.length) % pool.length; startRotation(); }
  function next() { featuredIndex = (featuredIndex + 1) % pool.length; startRotation(); }
  function goTo(i) { featuredIndex = i; startRotation(); }

  function timeAgo(ts) {
    const diff = Date.now() - ts;
    const m = Math.floor(diff / 60000);
    const h = Math.floor(diff / 3600000);
    const d = Math.floor(diff / 86400000);
    if (m < 1)  return 'Ahora';
    if (m < 60) return `${m}m`;
    if (h < 24) return `${h}h`;
    return `${d}d`;
  }

  // Convierte fecha ISO string a texto relativo para las cards
  function isoAgo(dateStr) {
    if (!dateStr) return '';
    const d = new Date(dateStr);
    if (isNaN(d)) return '';
    const diff = Date.now() - d.getTime();
    const min  = Math.floor(diff / 60000);
    const h    = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);
    if (min  <  1)  return 'Ahora';
    if (min  < 60)  return `Hace ${min}min`;
    if (h    < 24)  return `Hace ${h}h`;
    if (days <  1)  return 'Hace 1día';
    if (days <  7)  return `Hace ${days}días`;
    const weeks = Math.floor(days / 7);
    if (weeks < 4)  return weeks === 1 ? 'Hace 1semana' : `Hace ${weeks}semanas`;
    return '';
  }

  // ── Estadísticas ─────────────────────────────────────────────────────────
  let seriesCount  = $derived($readHistory.length);
  let daysActive   = $derived(Object.keys($readStats.activeDays ?? {}).length);
  let favSource = $derived.by(() => {
    const counts = {};
    for (const e of $readHistory) {
      const src = e.manga.id.startsWith('cerberus-') ? 'Cerberus'
                : e.manga.id.startsWith('taurus-')   ? 'Taurus'
                : e.manga.id.startsWith('leer-')      ? 'LeerCap'
                : 'Olympus';
      counts[src] = (counts[src] || 0) + 1;
    }
    return Object.entries(counts).sort((a, b) => b[1] - a[1])[0]?.[0] ?? '—';
  });

  // ── Últimas actualizaciones por fuente ───────────────────────────────────
  let updatesOlympus  = $state([]);
  let updatesCerberus = $state([]);
  let updatesTaurus   = $state([]);
  let updatesLeer     = $state([]);
  let updatesAnimeFlv = $state([]);
  let loadingUpdates  = $state(false);

  async function loadUpdates() {
    if (cacheLoaded) {
      updatesOlympus  = cachedOlympus;
      updatesCerberus = cachedCerberus;
      updatesTaurus   = cachedTaurus;
      updatesLeer     = cachedLeer;
      updatesAnimeFlv = cachedAnimeFlv;
      return;
    }
    loadingUpdates = true;

    try {
      // Una sola llamada Rust que lanza las 4 fuentes en paralelo
      const data = await invoke('get_home_updates');

      cachedOlympus   = stampDates(data.olympus);
      cachedCerberus  = stampDates(data.cerberus);
      cachedTaurus    = stampDates(data.taurus);
      cachedLeer      = stampDates(data.leercapitulo);
      cachedAnimeFlv  = data.animeflv ?? [];

      updatesOlympus  = cachedOlympus;
      updatesCerberus = cachedCerberus;
      updatesTaurus   = cachedTaurus;
      updatesLeer     = cachedLeer;
      updatesAnimeFlv = cachedAnimeFlv;
      cacheLoaded     = true;

      // Actualiza el titleCache con todos los items del home
      titleCache.update(c => {
        [...data.olympus, ...data.cerberus, ...data.taurus, ...data.leercapitulo]
          .forEach(item => { if (item.id && item.title) c[item.id] = item.title; });
        return c;
      });
    } catch (e) {
      console.error('Error cargando home:', e);
    } finally {
      loadingUpdates = false;
    }
  }

  function openBannerChapter(ch) {
    // Aseguramos que el manga esté cargado antes de abrir el capítulo
    // para que el botón "volver" encuentre el detalle
    selectedManga.set(featured);
    currentManga.set(featuredDetail ?? featured);
    openChapter(ch, featured.id);
  }

  function openAnimeItem(item) {
    pendingAnime.set(item);
    currentView.set('anime');
  }

  function statusColor(s) {
    const v = (s || '').toLowerCase();
    if (v.includes('activo'))    return 'var(--green)';
    if (v.includes('pausado'))   return 'var(--orange)';
    if (v.includes('cancelado')) return 'var(--red)';
    return 'var(--gray)';
  }
</script>

<div class="home">

  <!-- ── Banner principal (arriba) ───────────────────────────────────── -->
  {#if featured}
    <div class="banner" style="--img: url('{featured.image}')">
      <div class="banner-bg"></div>
      <div class="banner-grad-bottom"></div>
      <div class="banner-grad-left"></div>

      <div class="banner-content">
        <img
          class="cover"
          src={featured.image}
          alt={featured.title}
          onerror={(e) => e.target.style.display='none'}
        />
        <div class="info">
          <h1 class="title">{featuredDetail?.title || featured.title}</h1>
          <div class="meta">
            {#if featured.type}
              <span class="type-tag">{featured.type.toUpperCase()}</span>
            {/if}
            {#if featured.status}
              <span class="status-dot" style="color:{statusColor(featured.status)}">● {featured.status}</span>
            {/if}
            {#if featuredDetail?.genres?.length}
              <span class="genres">{featuredDetail.genres.slice(0, 3).join(' · ')}</span>
            {/if}
          </div>
          {#if featuredDetail?.description}
            <p class="synopsis">{featuredDetail.description}</p>
          {:else if loadingDetail}
            <p class="synopsis muted">Cargando sinopsis...</p>
          {/if}
          <button class="btn-primary" onclick={() => openManga(featured)}>Leer Manwha</button>
        </div>

        <!-- ── Panel UP NEXT (derecha, dentro del flex) ── -->
        {#if featuredDetail?.chapters?.length}
          <div class="chapters-panel">
            <span class="chapters-label">UP NEXT</span>

            <!-- Primer capítulo destacado -->
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="chapter-featured" onclick={() => openBannerChapter(featuredDetail.chapters[0])}>
              <div class="ch-featured-left">
                <span class="ch-badge">Ch. {featuredDetail.chapters[0].number ?? 1}</span>
                <span class="ch-featured-title">{featuredDetail.chapters[0].title ?? 'Capítulo ' + (featuredDetail.chapters[0].number ?? 1)}</span>
              </div>
              <span class="ch-play">▶</span>
            </div>

            <div class="ch-separator"></div>

            <!-- Resto de capítulos -->
            <div class="chapters-list">
              {#each featuredDetail.chapters.slice(1, 5) as ch}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <div class="chapter-row" onclick={() => openBannerChapter(ch)}>
                  <span class="ch-badge-sm">Ch. {ch.number ?? ch.title}</span>
                  <span class="ch-row-title">{ch.title ?? 'Capítulo ' + ch.number}</span>
                </div>
              {/each}
            </div>

            <button class="btn-ver-todos" onclick={() => openManga(featured)}>Todos los capítulos →</button>
          </div>
        {/if}
      </div>

      <button class="nav-btn nav-prev" onclick={prev}>‹</button>
      <button class="nav-btn nav-next" onclick={next}>›</button>

      <div class="dots">
        {#each pool as _, i}
          <button class="dot" class:active={i === featuredIndex} onclick={() => goTo(i)}></button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- ── Banner Anime ─────────────────────────────────────────────────────── -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="anime-banner" onclick={() => currentView.set('anime')}>
    <div class="anime-banner-left">
      <div class="anime-icon-wrap">
        <svg class="anime-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M3.375 19.5h17.25m-17.25 0a1.125 1.125 0 0 1-1.125-1.125M3.375 19.5h1.5C5.496 19.5 6 18.996 6 18.375m-3.75.125v-10.5A1.125 1.125 0 0 1 3.375 7.5h.375m0 0h13.5m0 0h.375a1.125 1.125 0 0 1 1.125 1.125V18.375c0 .621-.504 1.125-1.125 1.125M6 18.375V7.875m0 0H3.75M6 7.875h12M6 7.875v10.5m12-10.5v10.5M18 7.875H6" />
          <path stroke-linecap="round" stroke-linejoin="round"
            d="m15 12-6-3.5v7L15 12Z" />
        </svg>
      </div>
      <div class="anime-banner-info">
        <span class="anime-soon-tag">AnimeFLV</span>
        <h3 class="anime-banner-title">Últimas Actualizaciones Anime</h3>
        <p class="anime-banner-sub">
          {#if updatesAnimeFlv.length > 0}
            {updatesAnimeFlv.length} episodios recientes · Ver todos →
          {:else}
            Episodios recientes de tus series favoritas, todo en un solo lugar.
          {/if}
        </p>
      </div>
    </div>
    <div class="anime-banner-right">
      <div class="anime-dots-row">
        {#if updatesAnimeFlv.length > 0}
          {#each updatesAnimeFlv.slice(0, 6) as item (item.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="anime-real-card" onclick={(e) => { e.stopPropagation(); openAnimeItem(item); }} title="{item.title} · {item.episode}">
              <img src={item.image} alt={item.title} onerror={(e) => e.target.style.display='none'} />
              <div class="anime-real-ep">{item.episode.replace(/Episodio\s*/i, 'Ep.')}</div>
            </div>
          {/each}
        {:else}
          {#each Array(6) as _, i}
            <div class="anime-placeholder-card" style="--delay:{i * 0.1}s"></div>
          {/each}
        {/if}
      </div>
    </div>
  </div>

  <!-- ── Últimas actualizaciones + Stats ──────────────────────────────────── -->
  <div class="updates-section">

    <!-- Cabecera compartida -->
    <div class="updates-header">
      <div class="updates-title-wrap">
        <h2 class="updates-title">Últimas actualizaciones</h2>
        <span class="refresh-status" class:refreshing>
          {#if refreshing}
            <span class="refresh-spinner"></span>
            <span class="refresh-text">Actualizando…</span>
          {:else if lastRefresh}
            <span class="refresh-dot"></span>
            <span class="refresh-text">Hace {timeAgo(lastRefresh)}</span>
          {/if}
        </span>
      </div>
      <h2 class="updates-title">Mis estadísticas</h2>
    </div>

    <!-- Cuerpo: dos columnas alineadas -->
    <div class="updates-body">

      <!-- Columna izquierda: fuentes -->
      <div class="updates-col">
        {#if loadingUpdates}
          <p class="updates-loading">Cargando...</p>
        {:else}
          {#if updatesOlympus.length > 0}
            <div class="source-block">
              <span class="source-tag olympus-tag">Olympus</span>
              <div class="updates-strip">
                {#each updatesOlympus as item (item.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                  <div class="update-card" onclick={() => openManga(item)}>
                    <div class="update-cover-wrap">
                      <img class="update-cover" src={item.image} alt={item.title}
                        onerror={(e) => e.target.style.display='none'} />
                      {#if item.date || item.chapter}
                        <div class="update-badge-bottom">
                          <!-- Slot izquierda: capítulo más nuevo -->
                          <div class="update-slot">
                            <span class="update-chap">{item.chapter || 'Nuevo'}</span>
                            <span class="update-time">
                              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="6" cy="6" r="4.5"/><path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/></svg>
                              {isoAgo(item.date) || 'Ahora'}
                            </span>
                          </div>
                          <!-- Slot derecha: capítulo anterior (solo si existe) -->
                          {#if item.chapter2}
                            <div class="update-slot update-slot-prev">
                              <span class="update-chap update-chap-prev">{item.chapter2}</span>
                              {#if isoAgo(item.date2)}
                                <span class="update-time update-time-prev">{isoAgo(item.date2)}</span>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </div>
                    <p class="update-title">{item.title}</p>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          {#if updatesCerberus.length > 0}
            <div class="source-block">
              <span class="source-tag cerberus-tag">CerberusScan</span>
              <div class="updates-strip">
                {#each updatesCerberus as item (item.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                  <div class="update-card" onclick={() => openManga(item)}>
                    <div class="update-cover-wrap">
                      <img class="update-cover" src={item.image} alt={item.title}
                        onerror={(e) => e.target.style.display='none'} />
                      {#if item.date || item.chapter}
                        <div class="update-badge-bottom">
                          <!-- Slot izquierda: capítulo más nuevo -->
                          <div class="update-slot">
                            <span class="update-chap">{item.chapter || 'Nuevo'}</span>
                            <span class="update-time">
                              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="6" cy="6" r="4.5"/><path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/></svg>
                              {isoAgo(item.date) || 'Ahora'}
                            </span>
                          </div>
                          <!-- Slot derecha: capítulo anterior (solo si existe) -->
                          {#if item.chapter2}
                            <div class="update-slot update-slot-prev">
                              <span class="update-chap update-chap-prev">{item.chapter2}</span>
                              {#if isoAgo(item.date2)}
                                <span class="update-time update-time-prev">{isoAgo(item.date2)}</span>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </div>
                    <p class="update-title">{item.title}</p>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          {#if updatesTaurus.length > 0}
            <div class="source-block">
              <span class="source-tag taurus-tag">TaurusScan</span>
              <div class="updates-strip">
                {#each updatesTaurus as item (item.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                  <div class="update-card" onclick={() => openManga(item)}>
                    <div class="update-cover-wrap">
                      <img class="update-cover" src={item.image} alt={item.title}
                        onerror={(e) => e.target.style.display='none'} />
                      {#if item.date || item.chapter}
                        <div class="update-badge-bottom">
                          <!-- Slot izquierda: capítulo más nuevo -->
                          <div class="update-slot">
                            <span class="update-chap">{item.chapter || 'Nuevo'}</span>
                            <span class="update-time">
                              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="6" cy="6" r="4.5"/><path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/></svg>
                              {isoAgo(item.date) || 'Ahora'}
                            </span>
                          </div>
                          <!-- Slot derecha: capítulo anterior (solo si existe) -->
                          {#if item.chapter2}
                            <div class="update-slot update-slot-prev">
                              <span class="update-chap update-chap-prev">{item.chapter2}</span>
                              {#if isoAgo(item.date2)}
                                <span class="update-time update-time-prev">{isoAgo(item.date2)}</span>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </div>
                    <p class="update-title">{item.title}</p>
                  </div>
                {/each}
              </div>
            </div>
          {:else}
            <div class="source-block">
              <span class="source-tag taurus-tag">TaurusScan</span>
              <div class="updates-empty">{loadingUpdates ? 'Cargando...' : 'Sin datos'}</div>
            </div>
          {/if}

          <div class="source-block">
            <span class="source-tag leer-tag">LeerCapitulo</span>
            {#if updatesLeer.length > 0}
              <div class="updates-strip">
                {#each updatesLeer as item (item.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                  <div class="update-card" onclick={() => openManga(item)}>
                    <div class="update-cover-wrap">
                      <img class="update-cover" src={item.image} alt={item.title}
                        onerror={(e) => e.target.style.display='none'} />
                      {#if item.date || item.chapter}
                        <div class="update-badge-bottom">
                          <!-- Slot izquierda: capítulo más nuevo -->
                          <div class="update-slot">
                            <span class="update-chap">{item.chapter || 'Nuevo'}</span>
                            <span class="update-time">
                              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="6" cy="6" r="4.5"/><path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/></svg>
                              {isoAgo(item.date) || 'Ahora'}
                            </span>
                          </div>
                          <!-- Slot derecha: capítulo anterior (solo si existe) -->
                          {#if item.chapter2}
                            <div class="update-slot update-slot-prev">
                              <span class="update-chap update-chap-prev">{item.chapter2}</span>
                              {#if isoAgo(item.date2)}
                                <span class="update-time update-time-prev">{isoAgo(item.date2)}</span>
                              {/if}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </div>
                    <p class="update-title">{item.title}</p>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="updates-empty">{loadingUpdates ? 'Cargando...' : 'Sin datos'}</div>
            {/if}
          </div>

          {#if updatesAnimeFlv.length > 0}
            <div class="source-block">
              <span class="source-tag flv-tag">AnimeFLV</span>
              <div class="updates-strip">
                {#each updatesAnimeFlv as item (item.id)}
                  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                  <div class="update-card" onclick={() => openAnimeItem(item)}>
                    <div class="update-cover-wrap">
                      <img class="update-cover" src={item.image} alt={item.title}
                        onerror={(e) => e.target.style.display='none'} />
                      {#if item.episode}
                        <div class="update-badge-bottom">
                          <div class="update-slot">
                            <span class="update-chap">{item.episode}</span>
                          </div>
                        </div>
                      {/if}
                    </div>
                    <p class="update-title">{item.title}</p>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Columna derecha: estadísticas -->
      <div class="stats-col">

        <div class="stats-grid">
          <div class="stat-card">
            <span class="stat-icon">📖</span>
            <span class="stat-value">{$readStats.chaptersTotal ?? 0}</span>
            <span class="stat-label">Capítulos leídos</span>
          </div>
          <div class="stat-card">
            <span class="stat-icon">📚</span>
            <span class="stat-value">{seriesCount}</span>
            <span class="stat-label">Series</span>
          </div>
          <div class="stat-card">
            <span class="stat-icon">📅</span>
            <span class="stat-value">{daysActive}</span>
            <span class="stat-label">Días activo</span>
          </div>
          <div class="stat-card">
            <span class="stat-icon">⭐</span>
            <span class="stat-value stat-value-sm">{favSource}</span>
            <span class="stat-label">Fuente fav.</span>
          </div>
        </div>

        {#if $readHistory.length > 0}
          <div class="stats-recent">
            <p class="stats-recent-label">Leídos recientemente</p>
            {#each $readHistory.slice(0, 6) as entry (entry.manga.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="stats-row" onclick={() => openManga(entry.manga)}>
                <img class="stats-thumb" src={entry.manga.image} alt={entry.manga.title}
                  onerror={(e) => e.target.style.display='none'} />
                <div class="stats-row-info">
                  <p class="stats-row-title">{entry.manga.title}</p>
                  <p class="stats-row-ch">{entry.chapter.title}</p>
                </div>
                <span class="stats-row-time">{timeAgo(entry.timestamp)}</span>
              </div>
            {/each}
          </div>
        {/if}

      </div>
    </div>
  </div>

</div>

<style>
  .home {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }


  .muted { color: var(--text-muted); font-size: 13px; }

  /* ── Banner (arriba) ── */
  .banner {
    position: relative;
    height: 45%;
    max-height: 340px;
    flex-shrink: 0;
    overflow: hidden;
    margin: 6px 16px 6px;
    border-radius: 12px;
    border: 1px solid var(--outline-dim);
  }

  @media (min-height: 700px) {
    .banner { margin: 2px 16px; }
  }

  .banner-bg {
    position: absolute;
    inset: -8%;
    background-image: var(--img);
    background-size: cover;
    background-position: center top;
    filter: blur(28px) brightness(0.35) saturate(1.2);
  }

  .banner-grad-bottom {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgba(2,4,32,1) 0%, rgba(2,4,32,0.6) 40%, rgba(2,4,32,0.1) 100%);
  }

  .banner-grad-left {
    position: absolute;
    inset: 0;
    background: linear-gradient(to right, rgba(2,4,32,0.9) 0%, rgba(2,4,32,0.4) 50%, transparent 100%);
  }

  /* Contenido: cover a la izquierda, info a la derecha */
  .banner-content {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: clamp(10px, 2vw, 24px);
    padding: clamp(10px, 2vh, 20px) clamp(14px, 2.5vw, 32px);
    overflow: hidden;
  }

  .cover {
    height: clamp(70%, 80%, 90%);
    max-height: 210px;
    min-height: 80px;
    width: auto;
    aspect-ratio: 2/3;
    object-fit: cover;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.85);
    flex-shrink: 0;
  }

  /* Info ocupa el resto del ancho */
  .info {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: clamp(4px, 1vh, 10px);
    overflow: hidden;
  }

  .type-tag {
    display: inline-block;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--secondary);
    border: 1px solid var(--secondary);
    padding: 2px 8px;
    border-radius: 3px;
    width: fit-content;
  }

  .title {
    font-size: clamp(14px, 2.5vw, 32px);
    font-weight: 800;
    color: #fff;
    line-height: 1.2;
    text-shadow: 0 2px 16px rgba(0,0,0,0.9);
    word-break: break-word;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .status-dot { font-size: clamp(10px, 1.2vw, 13px); font-weight: 600; }
  .genres { font-size: clamp(10px, 1.1vw, 12px); color: var(--text-muted); }

  .synopsis {
    font-size: clamp(11px, 1.2vw, 13px);
    color: rgba(226,232,240,0.85);
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 1px 4px rgba(0,0,0,0.6);
  }

  .btn-primary {
    background: var(--primary);
    color: var(--on-primary);
    border: none;
    padding: 7px 16px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    width: fit-content;
    transition: opacity 0.15s, transform 0.1s;
  }
  .btn-primary:hover { opacity: 0.88; transform: translateY(-1px); }

  /* ── Panel UP NEXT (columna derecha dentro del flex) ── */
  .chapters-panel {
    width: clamp(140px, 18%, 200px);
    flex-shrink: 0;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    overflow: hidden;
    min-height: 0;
    padding: 4px 0;
  }

  .chapters-label {
    font-size: 10px;
    font-weight: 800;
    color: var(--primary);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    flex-shrink: 0;
    margin-bottom: 4px;
  }

  /* Primer cap destacado */
  .chapter-featured {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 10px;
    border-radius: 6px;
    background: rgba(245,158,11,0.12);
    border: 1px solid rgba(245,158,11,0.3);
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s;
  }
  .chapter-featured:hover { background: rgba(245,158,11,0.22); }

  .ch-featured-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .ch-badge {
    font-size: 9px;
    font-weight: 700;
    color: var(--primary);
    background: rgba(245,158,11,0.15);
    border: 1px solid rgba(245,158,11,0.4);
    padding: 2px 5px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .ch-featured-title {
    font-size: 11px;
    font-weight: 600;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ch-play {
    font-size: 10px;
    color: var(--primary);
    flex-shrink: 0;
    margin-left: 6px;
  }

  .ch-separator {
    height: 1px;
    background: rgba(255,255,255,0.08);
    flex-shrink: 0;
    margin: 2px 0;
  }

  /* Resto de capítulos */
  .chapters-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-evenly;
    overflow: hidden;
    min-height: 0;
    gap: 2px;
  }

  .chapter-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s;
    min-width: 0;
  }
  .chapter-row:hover { background: rgba(255,255,255,0.06); }

  .ch-badge-sm {
    font-size: 9px;
    font-weight: 700;
    color: var(--text-muted);
    white-space: nowrap;
    flex-shrink: 0;
    min-width: 36px;
  }

  .ch-row-title {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .btn-ver-todos {
    flex-shrink: 0;
    margin-top: 4px;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 0;
    cursor: pointer;
    text-align: left;
    transition: color 0.15s;
  }
  .btn-ver-todos:hover { color: var(--primary); }

  .nav-btn {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    background: rgba(2,4,32,0.5);
    border: 1px solid rgba(255,255,255,0.1);
    color: #fff;
    font-size: 24px;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s;
    line-height: 1;
  }
  .nav-btn:hover { background: rgba(245,158,11,0.4); }
  .nav-prev { left: 10px; }
  .nav-next { right: 10px; }

  .dots {
    position: absolute;
    bottom: 12px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 5px;
    align-items: center;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgba(255,255,255,0.3);
    border: none;
    cursor: pointer;
    padding: 0;
    transition: background 0.2s, transform 0.2s;
  }
  .dot.active { background: var(--primary); transform: scale(1.5); }

  /* ── Últimas actualizaciones + Stats ── */
  .updates-section {
    flex: 1;
    min-height: 0;
    border-top: 1px solid var(--outline-dim);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Cabecera compartida: dos títulos alineados con las columnas */
  .updates-header {
    display: flex;
    flex-shrink: 0;
    padding: 10px 0 0;
    border-bottom: 1px solid var(--outline-dim);
  }
  .updates-title-wrap {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 0 16px 8px;
  }
  .updates-title-wrap .updates-title { padding: 0; }
  .refresh-status {
    display: flex;
    align-items: center;
    gap: 5px;
    opacity: 0.55;
    font-size: 0.72rem;
    color: var(--text-dim, #8a9aae);
    transition: opacity 0.3s;
  }
  .refresh-status.refreshing { opacity: 0.9; color: var(--accent, #58a6ff); }
  .refresh-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: #4caf50;
    flex-shrink: 0;
  }
  .refresh-spinner {
    width: 10px; height: 10px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    flex-shrink: 0;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .refresh-text { white-space: nowrap; }
  .updates-header .updates-title:last-child {
    width: 38%;
    flex-shrink: 0;
    padding: 0 16px 8px;
    border-left: 1px solid var(--outline-dim);
  }

  /* Cuerpo: dos columnas lado a lado */
  .updates-body {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  /* Columna izquierda: actualizaciones */
  .updates-col {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 12px 8px 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    border-right: 1px solid var(--outline-dim);
  }

  /* Columna derecha: stats */
  .stats-col {
    width: 38%;
    flex-shrink: 0;
    overflow-y: auto;
    padding: 12px 16px 14px 8px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .updates-title {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .updates-loading {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Grid de stats — 2×2, tarjetas grandes */
  .stats-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .stat-card {
    background: var(--bg-card);
    border: 1px solid var(--outline-dim);
    border-radius: 8px;
    padding: 8px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    transition: border-color 0.15s, background 0.15s;
  }
  .stat-card:hover { border-color: var(--primary); background: var(--bg-card-high, var(--bg-card)); }

  .stat-icon { font-size: 14px; margin-bottom: 1px; }

  .stat-value {
    font-size: 18px;
    font-weight: 800;
    color: var(--text);
    line-height: 1;
  }
  .stat-value-sm { font-size: 11px; font-weight: 800; }

  .stat-label {
    font-size: 9px;
    color: var(--text-muted);
    text-align: center;
    margin-top: 1px;
  }

  /* Lista reciente compacta */
  .stats-recent {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stats-recent-label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    margin-bottom: 6px;
  }

  .stats-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 6px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.12s;
  }
  .stats-row:hover { background: var(--bg-card); }

  .stats-thumb {
    width: 34px;
    height: 48px;
    object-fit: cover;
    border-radius: 4px;
    flex-shrink: 0;
    border: 1px solid var(--outline-dim);
  }

  .stats-row-info {
    flex: 1;
    min-width: 0;
  }

  .stats-row-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .stats-row-ch {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
  }

  .stats-row-time {
    font-size: 9px;
    color: var(--outline);
    flex-shrink: 0;
  }

  .source-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .source-tag {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 2px 9px;
    border-radius: 12px;
    width: fit-content;
  }
  .olympus-tag  { background: rgba(99,102,241,0.15);  color: #818cf8; border: 1px solid rgba(99,102,241,0.3); }
  .cerberus-tag { background: rgba(239,68,68,0.12);   color: #f87171; border: 1px solid rgba(239,68,68,0.25); }
  .taurus-tag   { background: rgba(34,197,94,0.12);   color: #4ade80; border: 1px solid rgba(34,197,94,0.25); }
  .leer-tag     { background: rgba(234,179,8,0.12);   color: #facc15; border: 1px solid rgba(234,179,8,0.25); }
  .flv-tag      { background: rgba(168,85,247,0.12);  color: #c084fc; border: 1px solid rgba(168,85,247,0.25); }

  .updates-empty {
    font-size: 11px;
    color: var(--outline);
    font-style: italic;
    padding: 6px 2px;
  }

  .updates-strip {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 16px;
    scrollbar-width: thin;
    scrollbar-color: var(--outline-dim) transparent;
  }
  .updates-strip::-webkit-scrollbar { height: 4px; }
  .updates-strip::-webkit-scrollbar-thumb { background: var(--outline-dim); border-radius: 2px; }

  .update-card {
    flex-shrink: 0;
    width: 140px;
    cursor: pointer;
    transition: transform 0.15s;
  }
  .update-card:hover { transform: translateY(-2px); }

  .update-cover-wrap {
    width: 140px;
    height: 196px;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    position: relative;
  }

  .update-badge-bottom {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: linear-gradient(to top, rgba(0,0,0,0.92) 0%, rgba(0,0,0,0.6) 55%, transparent 100%);
    padding: 22px 5px 5px;
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: flex-end;
    gap: 2px;
  }

  /* Cada slot es una columna con capítulo + tiempo */
  .update-slot {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  /* Slot del capítulo anterior alineado a la derecha */
  .update-slot-prev {
    align-items: flex-end;
  }

  .update-chap {
    font-size: 10px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
    background: rgba(255,255,255,0.1);
    padding: 2px 5px;
    border-radius: 3px;
  }

  /* Capítulo anterior en color más tenue */
  .update-chap-prev {
    color: rgba(255,255,255,0.5);
    font-weight: 600;
    font-size: 9px;
    background: rgba(255,255,255,0.06);
  }

  .update-time {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 9.5px;
    color: #e879f9;
    white-space: nowrap;
    font-weight: 600;
  }
  .update-time svg { width: 9px; height: 9px; flex-shrink: 0; stroke: #e879f9; }

  /* Tiempo del cap anterior más apagado */
  .update-time-prev {
    color: rgba(232,121,249,0.45);
    justify-content: flex-end;
  }
  .update-time-prev svg { stroke: rgba(232,121,249,0.45); }

  .update-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .update-title {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 5px;
    line-height: 1.3;
  }

  /* ── Banner Anime ── */
  .anime-banner {
    flex-shrink: 0;
    margin: 0 16px 6px;
    border-radius: 10px;
    border: 1px solid rgba(139,92,246,0.25);
    background: linear-gradient(135deg, rgba(139,92,246,0.08) 0%, rgba(59,130,246,0.06) 50%, rgba(2,4,32,0.4) 100%);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    gap: 12px;
    overflow: hidden;
    position: relative;
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
  }
  .anime-banner:hover {
    border-color: rgba(139,92,246,0.5);
    background: linear-gradient(135deg, rgba(139,92,246,0.13) 0%, rgba(59,130,246,0.09) 50%, rgba(2,4,32,0.4) 100%);
  }

  /* Brillo sutil de fondo */
  .anime-banner::before {
    content: '';
    position: absolute;
    top: -40px;
    left: -20px;
    width: 180px;
    height: 180px;
    background: radial-gradient(circle, rgba(139,92,246,0.12) 0%, transparent 70%);
    pointer-events: none;
  }

  .anime-banner-left {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    flex: 1;
  }

  .anime-icon-wrap {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: rgba(139,92,246,0.15);
    border: 1px solid rgba(139,92,246,0.3);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .anime-icon {
    width: 20px;
    height: 20px;
    color: #a78bfa;
  }

  .anime-banner-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .anime-soon-tag {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #a78bfa;
    background: rgba(139,92,246,0.15);
    border: 1px solid rgba(139,92,246,0.3);
    padding: 1px 7px;
    border-radius: 10px;
    width: fit-content;
  }

  .anime-banner-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .anime-banner-sub {
    font-size: 10px;
    color: var(--text-muted);
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Tarjetas placeholder a la derecha */
  .anime-banner-right {
    flex-shrink: 0;
  }

  .anime-dots-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .anime-real-card {
    width: 44px;
    height: 62px;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid rgba(139,92,246,0.3);
    flex-shrink: 0;
    position: relative;
    cursor: pointer;
    transition: transform 0.15s, border-color 0.15s;
  }
  .anime-real-card:hover { transform: translateY(-2px); border-color: rgba(168,85,247,0.7); }
  .anime-real-card img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .anime-real-ep {
    position: absolute;
    bottom: 0; left: 0; right: 0;
    background: linear-gradient(to top, rgba(0,0,0,0.88) 0%, transparent 100%);
    font-size: 7px; font-weight: 700; color: #c084fc;
    padding: 4px 2px 2px;
    text-align: center;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .anime-placeholder-card {
    width: 44px;
    height: 62px;
    border-radius: 6px;
    background: linear-gradient(135deg, rgba(139,92,246,0.12), rgba(59,130,246,0.08));
    border: 1px solid rgba(139,92,246,0.15);
    animation: anime-pulse 2s ease-in-out infinite;
    animation-delay: var(--delay);
  }

  @keyframes anime-pulse {
    0%, 100% { opacity: 0.4; }
    50%       { opacity: 0.8; }
  }
</style>
