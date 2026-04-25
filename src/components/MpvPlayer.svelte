<script>
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  /** @type {{ url: string, fileName?: string, onclose?: () => void }} */
  let { url, fileName = '', onclose } = $props();

  // ── Estado del reproductor ────────────────────────────────────────────────
  let timePos   = $state(0);
  let duration  = $state(0);
  let paused    = $state(false);
  let volume    = $state(100);
  let loading   = $state(true);
  let error     = $state('');
  let seeking   = $state(false);
  let embedded  = $state(false); // true = mpv incrustado en ventana (X11/Win), false = externo (Wayland)

  // ── Subtítulos ────────────────────────────────────────────────────────────
  let subTracks    = $state([]);
  let activeSubId  = $state(0);   // 0 = sin subs
  let showSubMenu  = $state(false);
  let loadingSubs  = $state(false);

  // ── Tauri event listeners ─────────────────────────────────────────────────
  let unlisten = [];

  onMount(async () => {
    unlisten.push(await listen('mpv://time-pos', e  => {
      if (loading) { loading = false; loadSubTracks(); }
      timePos = e.payload ?? 0;
    }));
    unlisten.push(await listen('mpv://duration',  e  => { duration = e.payload ?? 0; }));
    unlisten.push(await listen('mpv://pause',     e  => { paused   = e.payload ?? false; }));
    unlisten.push(await listen('mpv://eof',       () => onclose?.()));

    try {
      embedded = await invoke('mpv_open', { path: url });
    } catch (e) {
      error = String(e);
      loading = false;
    }
  });

  onDestroy(async () => {
    unlisten.forEach(u => u());
    try { await invoke('mpv_close'); } catch {}
  });

  // ── Controles ─────────────────────────────────────────────────────────────
  async function togglePlay() {
    try { await invoke('mpv_pause_toggle'); } catch {}
  }

  async function seekTo(e) {
    if (duration <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const pct  = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const pos  = pct * duration;
    seeking = true;
    try {
      await invoke('mpv_seek', { pos });
    } catch {}
    seeking = false;
  }

  async function setVolume(e) {
    volume = Number(e.currentTarget.value);
    try { await invoke('mpv_set_volume', { vol: volume }); } catch {}
  }

  async function closePlayer() {
    try { await invoke('mpv_close'); } catch {}
    onclose?.();
  }

  async function loadSubTracks() {
    loadingSubs = true;
    try {
      subTracks = await invoke('mpv_get_tracks');
      const sel = subTracks.find(t => t.selected);
      activeSubId = sel ? sel.id : 0;
    } catch { subTracks = []; }
    finally { loadingSubs = false; }
  }

  async function selectSub(id) {
    activeSubId = id;
    showSubMenu = false;
    try { await invoke('mpv_set_sub', { id }); } catch {}
  }

  function fmtTime(s) {
    if (!s || isNaN(s)) return '0:00';
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2,'0')}:${String(sec).padStart(2,'0')}`
      : `${m}:${String(sec).padStart(2,'0')}`;
  }

  let pct = $derived(duration > 0 ? (timePos / duration) * 100 : 0);
</script>

<div class="mpv-wrap" class:mpv-wrap-embedded={embedded && !loading && !error}>
  <!-- Pantalla / estado -->
  <div class="mpv-screen" class:mpv-screen-transparent={embedded && !loading && !error}>
    {#if loading && !error}
      <div class="mpv-status">
        <div class="mpv-spinner"></div>
        <span>Abriendo reproductor…</span>
      </div>
    {:else if error}
      <div class="mpv-status mpv-err">{error}</div>
    {:else if !embedded}
      <div class="mpv-status mpv-playing">
        <svg viewBox="0 0 24 24" fill="currentColor" class="mpv-icon">
          <path d="M8 5v14l11-7z"/>
        </svg>
        <span>Reproduciendo en ventana mpv (Wayland)</span>
      </div>
    {/if}
  </div>

  <!-- Controles -->
  <div class="mpv-controls">
    <!-- Barra de progreso -->
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <div
      class="seek-bar"
      role="slider"
      aria-valuenow={pct}
      tabindex="0"
      onclick={seekTo}
      onkeydown={(e) => e.key === 'Enter' && seekTo(e)}
    >
      <div class="seek-track">
        <div class="seek-fill" style="width:{pct}%"></div>
        <div class="seek-thumb" style="left:{pct}%"></div>
      </div>
    </div>

    <!-- Botones -->
    <div class="ctrl-row">
      <!-- Play / Pause -->
      <button class="ctrl-btn" onclick={togglePlay} title={paused ? 'Reproducir' : 'Pausar'}>
        {#if paused}
          <svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
        {/if}
      </button>

      <!-- Tiempo -->
      <span class="ctrl-time">{fmtTime(timePos)} / {fmtTime(duration)}</span>

      <div class="ctrl-spacer"></div>

      <!-- Subtítulos CC -->
      <div class="cc-wrap">
        <button
          class="ctrl-btn cc-btn"
          class:cc-active={activeSubId !== 0}
          onclick={() => showSubMenu = !showSubMenu}
          title="Subtítulos"
        >
          {#if loadingSubs}
            <div class="cc-spinner"></div>
          {:else}
            <svg viewBox="0 0 24 24" fill="currentColor"><path d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm-9 8H9.5v-.5h-2v3h2V14H11v1c0 .55-.45 1-1 1H7c-.55 0-1-.45-1-1v-4c0-.55.45-1 1-1h3c.55 0 1 .45 1 1v1zm7 0h-1.5v-.5h-2v3h2V14H18v1c0 .55-.45 1-1 1h-3c-.55 0-1-.45-1-1v-4c0-.55.45-1 1-1h3c.55 0 1 .45 1 1v1z"/></svg>
          {/if}
          CC
        </button>
        {#if showSubMenu}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="cc-menu" onclick={(e) => e.stopPropagation()}>
            <button class="cc-item" class:cc-item-active={activeSubId === 0} onclick={() => selectSub(0)}>
              <span class="cc-dot"></span> Sin subtítulos
            </button>
            {#each subTracks as t}
              <button class="cc-item" class:cc-item-active={activeSubId === t.id} onclick={() => selectSub(t.id)}>
                <span class="cc-dot"></span>
                {t.title || t.lang || `Pista ${t.id}`}
                {#if t.lang}<span class="cc-lang">{t.lang.toUpperCase()}</span>{/if}
                {#if t.external}<span class="cc-ext">EXT</span>{/if}
              </button>
            {/each}
            {#if subTracks.length === 0 && !loadingSubs}
              <div class="cc-empty">Sin pistas detectadas</div>
            {/if}
            <button class="cc-item cc-rescan" onclick={loadSubTracks}>↻ Recargar pistas</button>
          </div>
        {/if}
      </div>

      <!-- Volumen -->
      <div class="vol-wrap">
        <svg viewBox="0 0 24 24" fill="currentColor" class="vol-icon">
          <path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3A4.5 4.5 0 0 0 14 7.97v8.05c1.48-.73 2.5-2.25 2.5-4.02z"/>
        </svg>
        <input
          type="range" min="0" max="100" step="1"
          bind:value={volume}
          oninput={setVolume}
          class="vol-slider"
          title="Volumen"
        />
      </div>

      <!-- Cerrar -->
      <button class="ctrl-btn ctrl-close" onclick={closePlayer} title="Cerrar reproductor">
        <svg viewBox="0 0 24 24" fill="currentColor">
          <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
        </svg>
      </button>
    </div>
  </div>

  {#if fileName}
    <div class="mpv-title">{fileName}</div>
  {/if}
</div>

<style>
  .mpv-wrap {
    display: flex;
    flex-direction: column;
    background: #0a0a0a;
    border-radius: 8px;
    overflow: hidden;
    width: 100%;
    user-select: none;
  }
  /* En modo incrustado el fondo es transparente para que mpv se vea */
  .mpv-wrap-embedded {
    background: transparent;
  }

  /* ── Pantalla ── */
  .mpv-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    background: #111;
  }
  /* Modo incrustado: sin fondo para que mpv se vea debajo del WebKit */
  .mpv-screen-transparent {
    background: transparent !important;
    min-height: 400px;
  }
  .mpv-status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: rgba(255,255,255,0.5);
    font-size: 13px;
  }
  .mpv-playing { color: rgba(255,255,255,0.75); }
  .mpv-err     { color: #f87171; }
  .mpv-icon    { width: 36px; height: 36px; opacity: 0.5; }
  .mpv-spinner {
    width: 28px; height: 28px;
    border: 3px solid rgba(255,255,255,0.1);
    border-top-color: var(--primary, #f59e0b);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Controles ── */
  .mpv-controls {
    padding: 8px 12px 10px;
    background: rgba(0,0,0,0.9);
    border-top: 1px solid rgba(255,255,255,0.07);
  }

  /* Barra seek */
  .seek-bar {
    cursor: pointer;
    padding: 6px 0;
    margin-bottom: 6px;
  }
  .seek-track {
    position: relative;
    height: 4px;
    background: rgba(255,255,255,0.15);
    border-radius: 2px;
  }
  .seek-fill {
    position: absolute;
    height: 100%;
    background: var(--primary, #f59e0b);
    border-radius: 2px;
    pointer-events: none;
  }
  .seek-thumb {
    position: absolute;
    top: 50%;
    width: 12px; height: 12px;
    background: #fff;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .seek-bar:hover .seek-thumb { opacity: 1; }

  /* Fila de botones */
  .ctrl-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ctrl-btn {
    background: none;
    border: none;
    color: rgba(255,255,255,0.8);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    transition: color 0.15s;
    flex-shrink: 0;
  }
  .ctrl-btn:hover { color: #fff; }
  .ctrl-btn svg  { width: 20px; height: 20px; }
  .ctrl-close svg { width: 18px; height: 18px; }
  .ctrl-close:hover { color: #f87171; }

  .ctrl-time {
    font-size: 12px;
    color: rgba(255,255,255,0.6);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .ctrl-spacer { flex: 1; }

  /* Volumen */
  .vol-wrap {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }
  .vol-icon { width: 16px; height: 16px; color: rgba(255,255,255,0.6); fill: currentColor; }
  .vol-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 72px; height: 4px;
    background: rgba(255,255,255,0.2);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
  .vol-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px; height: 12px;
    background: #fff;
    border-radius: 50%;
  }

  /* ── Subtítulos CC ── */
  .cc-wrap {
    position: relative;
    flex-shrink: 0;
  }

  .cc-btn {
    display: flex; align-items: center; gap: 4px;
    font-size: 10px; font-weight: 700;
    letter-spacing: 0.04em;
    padding: 3px 7px;
    border: 1px solid rgba(255,255,255,0.2);
    border-radius: 4px;
    color: rgba(255,255,255,0.7);
    transition: color 0.15s, border-color 0.15s, background 0.15s;
  }
  .cc-btn svg { width: 15px; height: 15px; }
  .cc-btn:hover { color: #fff; border-color: rgba(255,255,255,0.5); }
  .cc-btn.cc-active { color: var(--primary, #f59e0b); border-color: var(--primary, #f59e0b); }

  .cc-spinner {
    width: 12px; height: 12px;
    border: 2px solid rgba(255,255,255,0.2);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  .cc-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    right: 0;
    background: rgba(10,10,10,0.97);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 8px;
    overflow: hidden;
    min-width: 200px;
    max-height: 240px;
    overflow-y: auto;
    box-shadow: 0 6px 24px rgba(0,0,0,0.8);
    z-index: 100;
  }

  .cc-item {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 9px 14px;
    background: none; border: none;
    border-bottom: 1px solid rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.75);
    font-size: 12px; text-align: left; cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .cc-item:last-child { border-bottom: none; }
  .cc-item:hover { background: rgba(255,255,255,0.07); color: #fff; }
  .cc-item.cc-item-active { color: var(--primary, #f59e0b); font-weight: 700; }
  .cc-item.cc-item-active .cc-dot { background: var(--primary, #f59e0b); }
  .cc-item.cc-rescan {
    font-size: 10px; color: rgba(255,255,255,0.35);
    border-top: 1px solid rgba(255,255,255,0.08);
  }
  .cc-item.cc-rescan:hover { color: rgba(255,255,255,0.7); }

  .cc-dot {
    width: 6px; height: 6px; border-radius: 50%;
    background: rgba(255,255,255,0.2); flex-shrink: 0;
  }

  .cc-lang {
    font-size: 9px; font-weight: 800;
    background: rgba(245,158,11,0.15); color: var(--primary, #f59e0b);
    padding: 1px 5px; border-radius: 3px;
    letter-spacing: 0.05em; text-transform: uppercase;
    flex-shrink: 0; margin-left: auto;
  }
  .cc-ext {
    font-size: 9px; font-weight: 800;
    background: rgba(16,185,129,0.15); color: #34d399;
    padding: 1px 5px; border-radius: 3px;
    letter-spacing: 0.05em;
    flex-shrink: 0;
  }
  .cc-empty {
    padding: 10px 14px;
    font-size: 11px; color: rgba(255,255,255,0.3);
    text-align: center;
  }

  /* Título */
  .mpv-title {
    padding: 5px 12px 7px;
    font-size: 11px;
    color: rgba(255,255,255,0.35);
    background: rgba(0,0,0,0.9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-top: 1px solid rgba(255,255,255,0.04);
  }
</style>
