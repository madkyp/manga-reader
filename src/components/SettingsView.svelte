<script>
  import { onMount } from 'svelte';
  import { readHistory, readStats, downloadPath } from '../stores/manga.js';
  import { open } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { invoke } from '@tauri-apps/api/core';
  import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';

  // cli:true → soporta control directo via CLI instalado
  const VPN_LIST = [
    {
      id: 'proton',
      name: 'ProtonVPN',
      desc: 'Datos ilimitados en el plan gratuito · 3 países · Sin registros',
      url: 'https://protonvpn.com/download',
      color: '#6d28d9',
      badge: 'Gratis',
      cli: true,
    },
    {
      id: 'windscribe',
      name: 'Windscribe',
      desc: '10 GB/mes gratis · Múltiples países · Bloqueador de anuncios integrado',
      url: 'https://windscribe.com/download',
      color: '#0ea5e9',
      badge: '10 GB',
      cli: true,
    },
    {
      id: 'hideme',
      name: 'hide.me',
      desc: '10 GB/mes gratis · 5 ubicaciones · Sin registros de actividad',
      url: 'https://hide.me/en/software',
      color: '#10b981',
      badge: '10 GB',
      cli: false,
    },
  ];

  // Estado VPN: 'checking' | 'connected' | 'disconnected' | 'not_installed' | 'error'
  let vpnStatus  = $state({});
  let vpnLoading = $state({});

  async function refreshVpnStatus(id) {
    vpnStatus[id] = 'checking';
    vpnStatus[id] = await invoke('vpn_status', { provider: id }).catch(() => 'error');
  }

  async function connectVpn(id) {
    vpnLoading[id] = true;
    try {
      await invoke('vpn_connect', { provider: id });
    } catch {}
    vpnLoading[id] = false;
    await refreshVpnStatus(id);
  }

  async function disconnectVpn(id) {
    vpnLoading[id] = true;
    try {
      await invoke('vpn_disconnect', { provider: id });
    } catch {}
    vpnLoading[id] = false;
    await refreshVpnStatus(id);
  }


  // ── Carga preferencias desde localStorage ────────────────────────────────
  function loadPrefs() {
    try { return JSON.parse(localStorage.getItem('foundry_prefs') || '{}'); } catch { return {}; }
  }
  function savePrefs(p) {
    try { localStorage.setItem('foundry_prefs', JSON.stringify(p)); } catch {}
  }

  const prefs = loadPrefs();

  // Lector
  let readerWidth    = $state(prefs.readerWidth    ?? true);   // limitar ancho páginas
  let lazyLoad       = $state(prefs.lazyLoad       ?? true);   // carga diferida imágenes
  let showProgress   = $state(prefs.showProgress   ?? true);   // barra de progreso

  // Descargas
  let dlFormat       = $state(prefs.dlFormat === 'cbz' ? 'cbz' : 'pdf');  // 'pdf' | 'cbz'
  let confirmDlAll   = $state(prefs.confirmDlAll   ?? false);  // confirmar descargar todo

  async function pickDownloadFolder() {
    const selected = await open({ directory: true, multiple: false, title: 'Carpeta de descargas' });
    if (selected) downloadPath.set(selected);
  }

  function defaultDownloadPath() {
    return $downloadPath || '~/Downloads/Foundry';
  }

  // Fuentes
  let showDisabled   = $state(prefs.showDisabled   ?? false);  // mostrar fuentes desactivadas

  // Guarda automáticamente al cambiar cualquier toggle
  $effect(() => {
    savePrefs({ readerWidth, lazyLoad, showProgress, dlFormat, confirmDlAll, showDisabled });
  });

  // ── Acciones ─────────────────────────────────────────────────────────────
  let clearHistoryDone   = $state(false);
  let clearDlDone        = $state(false);

  function clearHistory() {
    readHistory.set([]);
    readStats.set({ chaptersTotal: 0, activeDays: {} });
    localStorage.removeItem('readHistory');
    localStorage.removeItem('readStats');
    clearHistoryDone = true;
    setTimeout(() => clearHistoryDone = false, 2000);
  }

  function clearDownloadMarkers() {
    for (const key of Object.keys(localStorage)) {
      if (key.startsWith('foundry_dl_')) localStorage.removeItem(key);
    }
    clearDlDone = true;
    setTimeout(() => clearDlDone = false, 2000);
  }

  const APP_VERSION = '0.1.0';

  // ── AniList ───────────────────────────────────────────────────────────────
  let anilistToken   = $state('');
  let anilistSaved   = $state(false);
  let anilistChecking = $state(false);
  let anilistConnected = $state(false);
  let anilistImporting = $state(false);
  let anilistImportDone = $state(false);

  async function loadAniListToken() {
    try {
      const t = await invoke('anilist_get_token');
      anilistToken = t ?? '';
      anilistConnected = !!t;
    } catch { anilistConnected = false; }
  }

  async function saveAniListToken() {
    anilistChecking = true;
    try {
      await invoke('anilist_save_token', { token: anilistToken.trim() });
      anilistConnected = !!anilistToken.trim();
      anilistSaved = true;
      setTimeout(() => anilistSaved = false, 2000);
    } catch (e) { console.error(e); }
    finally { anilistChecking = false; }
  }

  async function clearAniListToken() {
    await invoke('anilist_clear_token');
    anilistToken     = '';
    anilistConnected = false;
  }

  // ── Autostart ──────────────────────────────────────────────────────────────
  let autostart = $state(false);

  onMount(async () => {
    VPN_LIST.filter(v => v.cli).forEach(v => refreshVpnStatus(v.id));
    try { autostart = await isEnabled(); } catch {}
    await loadAniListToken();
  });

  async function toggleAutostart() {
    try {
      if (autostart) { await disable(); autostart = false; }
      else           { await enable();  autostart = true;  }
    } catch {}
  }
</script>

<div class="settings-wrap">
  <div class="settings-panel">

    <h1 class="settings-title">Opciones</h1>

    <!-- ── Lector ──────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 9h6M9 13h6M9 17h4"/></svg>
        Lector
      </h2>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Limitar ancho de páginas</span>
          <span class="row-desc">Centra y limita el ancho máximo al leer</span>
        </div>
        <button
          class="toggle"
          class:on={readerWidth}
          onclick={() => readerWidth = !readerWidth}
          aria-checked={readerWidth}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Carga diferida de imágenes</span>
          <span class="row-desc">Carga las páginas según haces scroll</span>
        </div>
        <button
          class="toggle"
          class:on={lazyLoad}
          onclick={() => lazyLoad = !lazyLoad}
          aria-checked={lazyLoad}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Barra de progreso</span>
          <span class="row-desc">Muestra el progreso de lectura en la parte superior</span>
        </div>
        <button
          class="toggle"
          class:on={showProgress}
          onclick={() => showProgress = !showProgress}
          aria-checked={showProgress}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>
    </section>

    <!-- ── Descargas ────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        Descargas
      </h2>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Carpeta de descargas</span>
          <span class="row-desc path-text">{defaultDownloadPath()}</span>
        </div>
        <button class="action-btn pick-btn" onclick={pickDownloadFolder}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
          Cambiar
        </button>
      </div>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Formato de descarga</span>
          <span class="row-desc">PDF (un documento) o CBZ (cómic, abre en cualquier lector)</span>
        </div>
        <div class="seg-ctrl">
          <button
            class="seg-btn"
            class:active={dlFormat === 'pdf'}
            onclick={() => dlFormat = 'pdf'}
          >PDF</button>
          <button
            class="seg-btn"
            class:active={dlFormat === 'cbz'}
            onclick={() => dlFormat = 'cbz'}
          >CBZ</button>
        </div>
      </div>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Confirmar "Descargar todo"</span>
          <span class="row-desc">Pide confirmación antes de descargar todos los capítulos</span>
        </div>
        <button
          class="toggle"
          class:on={confirmDlAll}
          onclick={() => confirmDlAll = !confirmDlAll}
          aria-checked={confirmDlAll}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>
    </section>

    <!-- ── Fuentes ───────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12"/></svg>
        Fuentes
      </h2>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Mostrar fuentes no disponibles</span>
          <span class="row-desc">Muestra Anime y Biblioteca en la barra lateral</span>
        </div>
        <button
          class="toggle"
          class:on={showDisabled}
          onclick={() => showDisabled = !showDisabled}
          aria-checked={showDisabled}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>
    </section>

    <!-- ── Datos ─────────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
        Datos
      </h2>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Historial de lectura</span>
          <span class="row-desc">Borra el historial y las estadísticas</span>
        </div>
        <button
          class="action-btn"
          class:done={clearHistoryDone}
          onclick={clearHistory}
        >
          {#if clearHistoryDone}
            <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
            Borrado
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
            Limpiar
          {/if}
        </button>
      </div>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Marcadores de descarga</span>
          <span class="row-desc">Elimina los ticks de capítulos descargados</span>
        </div>
        <button
          class="action-btn"
          class:done={clearDlDone}
          onclick={clearDownloadMarkers}
        >
          {#if clearDlDone}
            <svg viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414L8.414 15l-5.121-5.121a1 1 0 011.414-1.414L8.414 12.172l6.879-6.879a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
            Borrado
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
            Limpiar
          {/if}
        </button>
      </div>
    </section>

    <!-- ── VPN ─────────────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
        VPN
      </h2>

      <div class="vpn-note">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        ProtonVPN y Windscribe se pueden controlar directamente si tienes su CLI instalado.
      </div>

      {#each VPN_LIST as vpn}
        <div class="vpn-card">
          <div class="vpn-icon" style="background: {vpn.color}22; border-color: {vpn.color}44;">
            <svg viewBox="0 0 24 24" fill="none" stroke={vpn.color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
          </div>
          <div class="vpn-body">
            <div class="vpn-title-row">
              <span class="row-label">{vpn.name}</span>
              <span class="vpn-badge" style="background: {vpn.color}22; color: {vpn.color};">{vpn.badge}</span>
            </div>
            <span class="row-desc">{vpn.desc}</span>
            <div class="vpn-actions">
              {#if vpn.cli}
                {#if vpnLoading[vpn.id] || vpnStatus[vpn.id] === 'checking'}
                  <span class="vpn-state-badge loading"><span class="vpn-spinner"></span></span>
                {:else if vpnStatus[vpn.id] === 'connected'}
                  <span class="vpn-state-badge connected">● Conectado</span>
                  <button class="vpn-btn vpn-disconnect" onclick={() => disconnectVpn(vpn.id)}>Desconectar</button>
                {:else if vpnStatus[vpn.id] === 'disconnected'}
                  <span class="vpn-state-badge disconnected">○ Desconectado</span>
                  <button class="vpn-btn" style="--vpn-color: {vpn.color}" onclick={() => connectVpn(vpn.id)}>Conectar</button>
                {:else if vpnStatus[vpn.id] === 'not_installed'}
                  <span class="vpn-state-badge not-installed">Sin CLI</span>
                  <button class="vpn-btn" style="--vpn-color: {vpn.color}" onclick={() => openUrl(vpn.url)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                    Descargar
                  </button>
                {:else}
                  <button class="vpn-btn vpn-retry" onclick={() => refreshVpnStatus(vpn.id)}>↻ Reintentar</button>
                {/if}
              {:else}
                <button class="vpn-btn" style="--vpn-color: {vpn.color}" onclick={() => openUrl(vpn.url)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                  Descargar
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </section>

    <!-- ── AniList ──────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 8v4l3 3"/></svg>
        AniList
        {#if anilistConnected}
          <span class="al-badge-connected">Conectado</span>
        {/if}
      </h2>

      <p class="section-desc">
        Genera un token en <strong>anilist.co → Perfil → Ajustes → Desarrollador → Token de acceso personal</strong>.
        Úsalo para sincronizar el progreso de lectura.
      </p>

      <div class="al-token-row">
        <input
          class="al-token-input"
          type="password"
          placeholder="Token de acceso personal de AniList…"
          bind:value={anilistToken}
        />
        <button
          class="btn-save"
          class:saved={anilistSaved}
          onclick={saveAniListToken}
          disabled={anilistChecking || !anilistToken.trim()}
        >
          {anilistSaved ? '✓ Guardado' : anilistChecking ? '…' : 'Guardar'}
        </button>
        {#if anilistConnected}
          <button class="btn-danger-sm" onclick={clearAniListToken}>Desconectar</button>
        {/if}
      </div>

      {#if anilistConnected}
        <p class="al-hint">
          En cada manga de tu biblioteca encontrarás los botones <strong>AL +</strong> (vincular) y <strong>↑ Sync</strong> (sincronizar progreso).
        </p>
      {/if}
    </section>

    <!-- ── Sistema ───────────────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
        Sistema
      </h2>

      <div class="row">
        <div class="row-info">
          <span class="row-label">Iniciar con Windows</span>
          <span class="row-desc">Abre la app automáticamente al encender el PC</span>
        </div>
        <button
          class="toggle"
          class:on={autostart}
          onclick={toggleAutostart}
          aria-checked={autostart}
          role="switch"
        >
          <span class="thumb"></span>
        </button>
      </div>
    </section>

    <!-- ── Acerca de ─────────────────────────────────────────────────────── -->
    <section class="section about-section">
      <div class="about-row">
        <img src="/logo.jpg" alt="logo" class="about-logo" />
        <div class="about-text">
          <span class="about-name">The Foundry</span>
          <span class="about-ver">v{APP_VERSION}</span>
        </div>
      </div>
    </section>

  </div>
</div>

<style>
  .settings-wrap {
    flex: 1;
    overflow-y: auto;
    display: flex;
    justify-content: center;
    align-items: flex-start;   /* no estirar el panel a la altura de la ventana */
    padding: 32px 16px 48px;
    background: var(--bg);
  }

  .settings-panel {
    width: 100%;
    max-width: 560px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .settings-title {
    font-size: 20px;
    font-weight: 700;
    color: var(--text);
    margin: 0 0 12px;
    padding: 0 4px;
  }

  /* ── Sección ── */
  .section {
    background: var(--bg-card);
    border: 1px solid var(--outline-dim);
    border-radius: 12px;
    overflow: hidden;
    margin-bottom: 4px;
    flex-shrink: 0;   /* mantener altura natural; el scroll lo hace .settings-wrap */
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--primary);
    padding: 11px 16px 10px;
    border-bottom: 1px solid var(--outline-dim);
    margin: 0;
  }
  .section-title svg { width: 14px; height: 14px; flex-shrink: 0; }

  /* ── AniList ── */
  .al-badge-connected {
    font-size: 10px; font-weight: 700; padding: 1px 8px; border-radius: 10px;
    background: rgba(74,222,128,0.12); color: var(--green, #4ade80);
    border: 1px solid rgba(74,222,128,0.3);
  }
  .section-desc {
    font-size: 11px; color: var(--text-muted); margin: 0 0 10px; line-height: 1.5;
  }
  .al-token-row {
    display: flex; gap: 6px; align-items: center; flex-wrap: wrap;
  }
  .al-token-input {
    flex: 1; min-width: 160px; padding: 7px 10px;
    background: var(--bg); border: 1px solid var(--outline-dim);
    border-radius: 7px; color: var(--text); font-size: 12px; outline: none;
    transition: border-color 0.15s;
  }
  .al-token-input:focus { border-color: var(--primary); }
  .btn-save {
    padding: 7px 14px; border-radius: 7px; border: 1px solid var(--primary);
    background: color-mix(in srgb, var(--primary) 15%, transparent);
    color: var(--primary); font-size: 12px; font-weight: 600; cursor: pointer;
    white-space: nowrap; transition: background 0.15s;
  }
  .btn-save:hover { background: color-mix(in srgb, var(--primary) 25%, transparent); }
  .btn-save:disabled { opacity: 0.5; cursor: default; }
  .btn-save.saved { color: var(--green, #4ade80); border-color: var(--green, #4ade80); }
  .btn-danger-sm {
    padding: 7px 12px; border-radius: 7px; border: 1px solid rgba(248,113,113,0.4);
    background: rgba(248,113,113,0.08); color: var(--red, #f87171);
    font-size: 12px; font-weight: 600; cursor: pointer; white-space: nowrap;
  }
  .al-hint {
    font-size: 11px; color: var(--text-muted); margin: 8px 0 0; line-height: 1.5;
  }

  /* ── Fila ── */
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px 16px;
    padding: 13px 16px;
    border-bottom: 1px solid var(--outline-dim);
  }
  .row:last-child { border-bottom: none; }
  /* El control (toggle, segmentado, botón) queda a la derecha aunque la fila
     haga wrap en pantallas estrechas, en vez de cortarse. */
  .row > :not(.row-info) { margin-left: auto; flex-shrink: 0; }

  .row-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1 1 200px;
  }
  .row-label {
    font-size: 13px;
    color: var(--text);
    font-weight: 500;
  }
  .row-desc {
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.4;
  }

  /* ── Toggle switch ── */
  .toggle {
    width: 42px;
    height: 24px;
    border-radius: 12px;
    border: none;
    background: var(--outline-dim);
    cursor: pointer;
    padding: 3px;
    display: flex;
    align-items: center;
    transition: background 0.2s;
    flex-shrink: 0;
    position: relative;
  }
  .toggle.on { background: var(--primary); }

  .thumb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.2s;
    flex-shrink: 0;
    box-shadow: 0 1px 3px rgba(0,0,0,0.3);
  }
  .toggle.on .thumb { transform: translateX(18px); }

  /* ── Control segmentado (PDF / Imágenes) ── */
  .seg-ctrl {
    display: flex;
    border-radius: 7px;
    border: 1px solid var(--outline-dim);
    overflow: hidden;
    flex-shrink: 0;
  }
  .seg-btn {
    padding: 5px 12px;
    font-size: 11px;
    font-weight: 600;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .seg-btn + .seg-btn { border-left: 1px solid var(--outline-dim); }
  .seg-btn.active {
    background: var(--primary);
    color: #fff;
  }

  /* ── Botón de acción (limpiar) ── */
  .action-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border-radius: 7px;
    border: 1px solid var(--outline-dim);
    background: none;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    white-space: nowrap;
  }
  .action-btn svg { width: 14px; height: 14px; }
  .action-btn:hover { background: rgba(239,68,68,0.08); color: #f87171; border-color: rgba(239,68,68,0.3); }
  .action-btn.done { color: #4ade80; border-color: rgba(74,222,128,0.3); background: rgba(74,222,128,0.06); }
  .action-btn.pick-btn { color: var(--text-muted); }
  .action-btn.pick-btn:hover { background: var(--bg-card-high); color: var(--text); border-color: var(--primary); }

  .path-text {
    font-family: monospace;
    font-size: 10px;
    color: var(--text-muted);
    word-break: break-all;
    max-width: 280px;
  }

  /* ── VPN ── */
  .vpn-note {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    padding: 10px 16px;
    font-size: 11px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--outline-dim);
    line-height: 1.5;
  }
  .vpn-note svg { width: 13px; height: 13px; flex-shrink: 0; margin-top: 1px; }

  .vpn-card {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--outline-dim);
  }
  .vpn-card:last-child { border-bottom: none; }

  .vpn-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .vpn-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
  }

  .vpn-state-badge {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 5px;
    white-space: nowrap;
  }
  .vpn-state-badge.connected    { background: rgba(74,222,128,0.12); color: #4ade80; }
  .vpn-state-badge.disconnected { background: rgba(255,255,255,0.06); color: var(--text-muted); }
  .vpn-state-badge.not-installed{ background: rgba(255,255,255,0.05); color: var(--text-muted); }
  .vpn-state-badge.loading      { background: transparent; display: flex; align-items: center; padding: 2px 4px; }

  .vpn-spinner {
    width: 12px; height: 12px;
    border: 2px solid var(--outline-dim);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    display: inline-block;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .vpn-disconnect {
    --vpn-color: #f87171;
    border-color: color-mix(in srgb, var(--vpn-color) 35%, transparent);
    background: color-mix(in srgb, var(--vpn-color) 8%, transparent);
    color: var(--vpn-color);
  }
  .vpn-disconnect:hover {
    background: color-mix(in srgb, var(--vpn-color) 18%, transparent);
    border-color: color-mix(in srgb, var(--vpn-color) 60%, transparent);
  }
  .vpn-retry { --vpn-color: var(--text-muted); }

  .vpn-icon {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    border: 1px solid;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .vpn-icon svg { width: 18px; height: 18px; }

  .vpn-title-row {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .vpn-badge {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.05em;
    border-radius: 4px;
    padding: 1px 5px;
    text-transform: uppercase;
  }

  .vpn-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 11px;
    border-radius: 7px;
    border: 1px solid color-mix(in srgb, var(--vpn-color) 35%, transparent);
    background: color-mix(in srgb, var(--vpn-color) 8%, transparent);
    color: var(--vpn-color);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s, border-color 0.15s;
    white-space: nowrap;
  }
  .vpn-btn svg { width: 12px; height: 12px; }
  .vpn-btn:hover {
    background: color-mix(in srgb, var(--vpn-color) 18%, transparent);
    border-color: color-mix(in srgb, var(--vpn-color) 60%, transparent);
  }

  /* ── Acerca de ── */
  .about-section { }
  .about-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px;
  }
  .about-logo {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    object-fit: cover;
    flex-shrink: 0;
  }
  .about-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .about-name { font-size: 14px; font-weight: 700; color: var(--text); }
  .about-ver  { font-size: 11px; color: var(--text-muted); }
</style>
