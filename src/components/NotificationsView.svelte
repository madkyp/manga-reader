<script>
  import { onMount } from 'svelte';
  import {
    library, notifState, checkingNotifs, unreadCount,
    checkNotifications, markNotifRead, markAllNotifsRead, openManga
  } from '../stores/manga.js';
  import {
    animeLibrary, animeNotifState, checkingAnimeNotifs, animeUnreadCount,
    checkAnimeNotifications, markAnimeNotifRead, markAllAnimeNotifsRead, openAnimeFromLibrary
  } from '../stores/anime.js';

  let tab = $state('manga');

  let mangaNotifItems = $derived(
    $library
      .map(item => ({ item, notif: $notifState[item.id] }))
      .filter(({ notif }) => notif?.unread)
      .sort((a, b) => new Date(b.notif.checkedAt) - new Date(a.notif.checkedAt))
  );
  let mangaPendingItems = $derived($library.filter(item => !$notifState[item.id]));

  let animeNotifItems = $derived(
    $animeLibrary
      .map(item => ({ item, notif: $animeNotifState[item.id] }))
      .filter(({ notif }) => notif?.unread)
      .sort((a, b) => new Date(b.notif.checkedAt) - new Date(a.notif.checkedAt))
  );
  let animePendingItems = $derived($animeLibrary.filter(item => !$animeNotifState[item.id]));

  function relativeTime(iso) {
    if (!iso) return '';
    const diff = Date.now() - new Date(iso).getTime();
    const m = Math.floor(diff / 60000);
    if (m < 1)  return 'ahora mismo';
    if (m < 60) return `hace ${m}m`;
    const h = Math.floor(m / 60);
    if (h < 24) return `hace ${h}h`;
    return `hace ${Math.floor(h / 24)}d`;
  }

  function handleOpenManga(item) { markNotifRead(item.id); openManga(item); }
  function handleOpenAnime(item) { markAnimeNotifRead(item.id); openAnimeFromLibrary(item); }

  onMount(() => { checkNotifications(); checkAnimeNotifications(); });
</script>

<div class="notif-view">
  <div class="notif-header">
    <div class="title-row">
      <h1 class="notif-title">Notificaciones</h1>
      {#if ($unreadCount + $animeUnreadCount) > 0}
        <span class="badge">{$unreadCount + $animeUnreadCount}</span>
      {/if}
    </div>
    <div class="actions">
      {#if tab === 'manga' && $unreadCount > 0}
        <button class="btn-text" onclick={markAllNotifsRead}>Marcar todo leído</button>
      {:else if tab === 'anime' && $animeUnreadCount > 0}
        <button class="btn-text" onclick={markAllAnimeNotifsRead}>Marcar todo leído</button>
      {/if}
      <button class="btn-check"
        onclick={() => tab === 'manga' ? checkNotifications(true) : checkAnimeNotifications(true)}
        disabled={tab === 'manga' ? $checkingNotifs : $checkingAnimeNotifs}>
        {#if (tab === 'manga' ? $checkingNotifs : $checkingAnimeNotifs)}
          <svg class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          Revisando…
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          Revisar ahora
        {/if}
      </button>
    </div>
  </div>

  <div class="tabs">
    <button class="tab" class:active={tab === 'manga'} onclick={() => tab = 'manga'}>
      Manhwa / Manga
      {#if $unreadCount > 0}<span class="tab-badge">{$unreadCount}</span>{/if}
    </button>
    <button class="tab" class:active={tab === 'anime'} onclick={() => tab = 'anime'}>
      Anime
      {#if $animeUnreadCount > 0}<span class="tab-badge">{$animeUnreadCount}</span>{/if}
    </button>
  </div>

  <div class="scroll-area">
    {#if tab === 'manga'}
      {#if $library.length === 0}
        <div class="empty">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/>
            <path d="M13.73 21a2 2 0 0 1-3.46 0"/>
          </svg>
          <p>No hay nada en tu biblioteca</p>
          <span>Añade manwhas a tu biblioteca para recibir alertas</span>
        </div>
      {:else if mangaNotifItems.length === 0}
        <div class="empty">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
            <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/>
            <path d="M13.73 21a2 2 0 0 1-3.46 0"/>
          </svg>
          <p>Todo al día</p>
          <span>{$checkingNotifs ? 'Comprobando…' : 'No hay capítulos nuevos'}</span>
          {#if mangaPendingItems.length > 0 && !$checkingNotifs}
            <span class="pending-hint">{mangaPendingItems.length} manga aún sin revisar — pulsa "Revisar ahora"</span>
          {/if}
        </div>
      {:else}
        <div class="list">
          {#each mangaNotifItems as { item, notif } (item.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="notif-card" onclick={() => handleOpenManga(item)}>
              <div class="cover-wrap">
                <img class="cover" src={item.image} alt={item.title} loading="lazy"
                  onerror={(e) => e.target.src='https://picsum.photos/56/80'} />
                <div class="new-dot"></div>
              </div>
              <div class="info">
                <p class="item-title">{item.title}</p>
                <p class="sub-line">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                  {notif.lastChapterTitle || 'Nuevo capítulo'}
                </p>
                <p class="time">{relativeTime(notif.checkedAt)}</p>
              </div>
              <svg class="arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="9 18 15 12 9 6"/></svg>
            </div>
          {/each}
        </div>
      {/if}

    {:else}
      {#if $animeLibrary.length === 0}
        <div class="empty">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/>
          </svg>
          <p>Sin animes en biblioteca</p>
          <span>Añade animes a tu biblioteca para recibir alertas de nuevos episodios</span>
        </div>
      {:else if animeNotifItems.length === 0}
        <div class="empty">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/>
          </svg>
          <p>Todo al día</p>
          <span>{$checkingAnimeNotifs ? 'Comprobando…' : 'No hay episodios nuevos'}</span>
          {#if animePendingItems.length > 0 && !$checkingAnimeNotifs}
            <span class="pending-hint">{animePendingItems.length} anime aún sin revisar — pulsa "Revisar ahora"</span>
          {/if}
        </div>
      {:else}
        <div class="list">
          {#each animeNotifItems as { item, notif } (item.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="notif-card" onclick={() => handleOpenAnime(item)}>
              <div class="cover-wrap">
                <img class="cover" src={item.image} alt={item.title} loading="lazy"
                  onerror={(e) => e.target.src='https://picsum.photos/56/80'} />
                <div class="new-dot"></div>
              </div>
              <div class="info">
                <p class="item-title">{item.title}</p>
                <p class="sub-line">
                  <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>
                  Nuevo episodio: Ep. {notif.lastEpisode}
                </p>
                <p class="time">{relativeTime(notif.checkedAt)}</p>
              </div>
              <svg class="arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="9 18 15 12 9 6"/></svg>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .notif-view {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 16px 20px 0;
    gap: 12px;
  }

  .notif-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .title-row { display: flex; align-items: center; gap: 8px; }
  .notif-title { font-size: 16px; font-weight: 800; color: var(--text); }

  .badge {
    background: var(--primary, #f59e0b);
    color: #000;
    font-size: 10px;
    font-weight: 800;
    border-radius: 99px;
    padding: 1px 6px;
  }

  .actions { display: flex; align-items: center; gap: 8px; }

  .btn-text {
    font-size: 11px; color: var(--text-muted);
    background: none; border: none; cursor: pointer;
    padding: 4px 8px; border-radius: 4px; transition: color 0.15s;
  }
  .btn-text:hover { color: var(--text); }

  .btn-check {
    display: flex; align-items: center; gap: 5px;
    font-size: 11px; font-weight: 600; color: var(--text);
    background: var(--bg-card, rgba(255,255,255,0.05));
    border: 1px solid var(--outline-dim, rgba(255,255,255,0.08));
    border-radius: 6px; padding: 5px 10px; cursor: pointer;
    transition: opacity 0.15s;
  }
  .btn-check:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-check svg { width: 13px; height: 13px; }

  @keyframes spin { to { transform: rotate(360deg); } }
  .spin { animation: spin 0.8s linear infinite; }

  /* ── Tabs ── */
  .tabs {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--outline-dim);
    margin: 0 -20px;
    padding: 0 20px;
  }

  .tab {
    display: flex; align-items: center; gap: 5px;
    padding: 6px 14px;
    font-size: 12px; font-weight: 600;
    background: none; border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted); cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
    margin-bottom: -1px;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--primary); border-bottom-color: var(--primary); }

  .tab-badge {
    background: var(--primary, #f59e0b);
    color: #000;
    font-size: 9px; font-weight: 800;
    border-radius: 99px;
    padding: 1px 5px;
    line-height: 1.5;
  }

  .scroll-area { flex: 1; overflow-y: auto; padding-bottom: 16px; }

  .empty {
    height: 100%;
    display: flex; flex-direction: column;
    align-items: center; justify-content: center;
    gap: 10px; color: var(--text-muted);
    padding: 40px 20px; text-align: center;
  }
  .empty svg { width: 48px; height: 48px; opacity: 0.3; }
  .empty p { font-size: 14px; font-weight: 600; color: var(--text); margin: 0; }
  .empty span { font-size: 12px; }
  .pending-hint { font-size: 11px; color: var(--primary, #f59e0b); margin-top: 4px; }

  .list { display: flex; flex-direction: column; gap: 4px; }

  .notif-card {
    display: flex; align-items: center; gap: 12px;
    padding: 10px 12px; border-radius: 8px;
    cursor: pointer; transition: background 0.12s;
    border: 1px solid transparent;
  }
  .notif-card:hover {
    background: var(--bg-card, rgba(255,255,255,0.04));
    border-color: var(--outline-dim, rgba(255,255,255,0.07));
  }

  .cover-wrap {
    position: relative; flex-shrink: 0;
    width: 42px; height: 60px;
    border-radius: 4px; overflow: hidden;
    background: var(--bg-card);
  }
  .cover { width: 100%; height: 100%; object-fit: cover; display: block; }

  .new-dot {
    position: absolute; top: 4px; right: 4px;
    width: 8px; height: 8px; border-radius: 50%;
    background: var(--primary, #f59e0b);
    box-shadow: 0 0 6px var(--primary, #f59e0b);
  }

  .info {
    flex: 1; min-width: 0;
    display: flex; flex-direction: column; gap: 3px;
  }

  .item-title {
    font-size: 13px; font-weight: 700; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin: 0;
  }

  .sub-line {
    display: flex; align-items: center; gap: 4px;
    font-size: 11px; color: var(--text-muted);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin: 0;
  }
  .sub-line svg { width: 11px; height: 11px; flex-shrink: 0; }

  .time { font-size: 10px; color: #e879f9; margin: 0; }

  .arrow { flex-shrink: 0; width: 14px; height: 14px; opacity: 0.3; }
  .notif-card:hover .arrow { opacity: 0.7; }
</style>
