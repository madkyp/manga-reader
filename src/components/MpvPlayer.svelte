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

  // ── Tauri event listeners ─────────────────────────────────────────────────
  let unlisten = [];

  onMount(async () => {
    unlisten.push(await listen('mpv://time-pos', e  => { timePos  = e.payload ?? 0; loading = false; }));
    unlisten.push(await listen('mpv://duration',  e  => { duration = e.payload ?? 0; }));
    unlisten.push(await listen('mpv://pause',     e  => { paused   = e.payload ?? false; }));
    unlisten.push(await listen('mpv://eof',       () => onclose?.()));

    try {
      await invoke('mpv_open', { path: url });
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
    try { await invoke('mpv_set_volume', { vol: BigInt(volume) }); } catch {}
  }

  async function closePlayer() {
    try { await invoke('mpv_close'); } catch {}
    onclose?.();
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

<div class="mpv-wrap">
  <!-- Pantalla / estado -->
  <div class="mpv-screen">
    {#if loading && !error}
      <div class="mpv-status">
        <div class="mpv-spinner"></div>
        <span>Abriendo en mpv…</span>
      </div>
    {:else if error}
      <div class="mpv-status mpv-err">{error}</div>
    {:else}
      <div class="mpv-status mpv-playing">
        <svg viewBox="0 0 24 24" fill="currentColor" class="mpv-icon">
          <path d="M8 5v14l11-7z"/>
        </svg>
        <span>Reproduciendo en ventana mpv</span>
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

  /* ── Pantalla ── */
  .mpv-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    background: #111;
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
