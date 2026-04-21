<script>
  import { library, toggleLibrary, openManga } from '../stores/manga.js';
  import { animeLibrary, toggleAnimeLibrary, openAnimeFromLibrary } from '../stores/anime.js';

  let tab = $state('manga'); // 'manga' | 'anime'
</script>

<div class="library">
  <div class="library-header">
    <h1 class="library-title">Mi Biblioteca</h1>
    <span class="library-count">
      {tab === 'manga' ? $library.length : $animeLibrary.length}
      {tab === 'manga' ? 'serie' : 'anime'}{(tab === 'manga' ? $library.length : $animeLibrary.length) !== 1 ? 's' : ''}
    </span>
  </div>

  <div class="tabs">
    <button class="tab" class:active={tab === 'manga'} onclick={() => tab = 'manga'}>Manhwa / Manga</button>
    <button class="tab" class:active={tab === 'anime'} onclick={() => tab = 'anime'}>Anime</button>
  </div>

  {#if tab === 'manga'}
    {#if $library.length === 0}
      <div class="empty">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
          <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
        </svg>
        <p>Tu biblioteca está vacía</p>
        <span>Abre cualquier manwha y pulsa "Añadir a biblioteca"</span>
      </div>
    {:else}
      <div class="grid">
        {#each $library as item (item.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="card" onclick={() => openManga(item)}>
            <div class="cover-wrap">
              <img class="cover" src={item.image} alt={item.title} loading="lazy"
                onerror={(e) => e.target.src='https://picsum.photos/120/170'} />
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="remove-btn" title="Eliminar de biblioteca"
                onclick={(e) => { e.stopPropagation(); toggleLibrary(item); }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </div>
            </div>
            <p class="title" title={item.title}>{item.title}</p>
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
        <span>Abre un anime y pulsa "Añadir a biblioteca"</span>
      </div>
    {:else}
      <div class="grid">
        {#each $animeLibrary as item (item.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="card" onclick={() => openAnimeFromLibrary(item)}>
            <div class="cover-wrap">
              <img class="cover" src={item.image} alt={item.title} loading="lazy"
                onerror={(e) => e.target.src='https://picsum.photos/120/170'} />
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="remove-btn" title="Eliminar de biblioteca"
                onclick={(e) => { e.stopPropagation(); toggleAnimeLibrary(item); }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </div>
            </div>
            <p class="title" title={item.title}>{item.title}</p>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .library {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 16px 20px;
    gap: 12px;
  }

  .library-header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-shrink: 0;
  }

  .library-title { font-size: 16px; font-weight: 800; color: var(--text); }
  .library-count { font-size: 11px; color: var(--text-muted); font-weight: 600; }

  /* ── Tabs ── */
  .tabs {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--outline-dim);
    padding-bottom: 0;
  }

  .tab {
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
    margin-bottom: -1px;
  }
  .tab:hover { color: var(--text); }
  .tab.active { color: var(--primary); border-bottom-color: var(--primary); }

  /* ── Empty ── */
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-muted);
  }
  .empty svg { width: 48px; height: 48px; opacity: 0.3; }
  .empty p { font-size: 14px; font-weight: 600; color: var(--text); }
  .empty span { font-size: 12px; text-align: center; }

  /* ── Grid ── */
  .grid {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-content: flex-start;
    padding-bottom: 16px;
  }

  .card { display: flex; flex-direction: column; gap: 6px; cursor: pointer; width: 120px; }
  .card:hover .cover { opacity: 0.85; transform: scale(1.02); }

  .cover-wrap {
    position: relative;
    width: 120px;
    height: 170px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-card);
    border: 1px solid var(--outline-dim);
  }

  .cover {
    width: 100%; height: 100%;
    object-fit: cover; display: block;
    transition: opacity 0.15s, transform 0.15s;
  }

  .remove-btn {
    position: absolute;
    top: 5px; right: 5px;
    width: 22px; height: 22px;
    background: rgba(2,4,32,0.82);
    border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    opacity: 0;
    transition: opacity 0.15s, background 0.15s;
    cursor: pointer;
  }
  .remove-btn svg { width: 11px; height: 11px; stroke: #fff; }
  .card:hover .remove-btn { opacity: 1; }
  .remove-btn:hover { background: rgba(239,68,68,0.85); }

  .title {
    font-size: 11px; color: var(--text); line-height: 1.3;
    max-width: 120px;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
</style>
