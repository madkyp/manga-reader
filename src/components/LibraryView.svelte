<script>
  import { get } from 'svelte/store';
  import { library, toggleLibrary, openManga } from '../stores/manga.js';
  import { animeLibrary, toggleAnimeLibrary, openAnimeFromLibrary } from '../stores/anime.js';

  let tab      = $state('manga'); // 'manga' | 'anime'
  let feedback = $state('');      // mensaje temporal de éxito/error

  function showFeedback(msg) {
    feedback = msg;
    setTimeout(() => feedback = '', 3000);
  }

  async function exportLibrary() {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const { invoke } = await import('@tauri-apps/api/core');

    const data = {
      version: 1,
      exportedAt: new Date().toISOString(),
      manga: get(library),
      anime: get(animeLibrary),
    };

    const path = await save({
      defaultPath: `foundry-biblioteca-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (!path) return; // usuario canceló

    try {
      await invoke('write_text_file', { path, content: JSON.stringify(data, null, 2) });
      showFeedback(`Exportados: ${data.manga.length} manga, ${data.anime.length} anime`);
    } catch (e) {
      showFeedback(`Error al guardar: ${e}`);
    }
  }

  function importLibrary() {
    const input   = document.createElement('input');
    input.type    = 'file';
    input.accept  = '.json';
    input.onchange = async (e) => {
      const file = e.target.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const data = JSON.parse(text);

        const mangaItems = Array.isArray(data.manga) ? data.manga : [];
        const animeItems = Array.isArray(data.anime) ? data.anime : [];

        // Fusionar con la biblioteca existente (no reemplazar, añadir los que faltan)
        library.update(lib => {
          const ids  = new Set(lib.map(x => x.id));
          const next = [...lib, ...mangaItems.filter(x => !ids.has(x.id))];
          localStorage.setItem('foundry_library', JSON.stringify(next));
          return next;
        });

        animeLibrary.update(lib => {
          const ids  = new Set(lib.map(x => x.id));
          const next = [...lib, ...animeItems.filter(x => !ids.has(x.id))];
          localStorage.setItem('foundry_anime_library', JSON.stringify(next));
          return next;
        });

        showFeedback(`Importados: ${mangaItems.length} manga, ${animeItems.length} anime`);
      } catch {
        showFeedback('Error: archivo no válido');
      }
    };
    input.click();
  }
</script>

<div class="library">
  <div class="library-header">
    <h1 class="library-title">Mi Biblioteca</h1>
    <span class="library-count">
      {tab === 'manga' ? $library.length : $animeLibrary.length}
      {tab === 'manga' ? 'serie' : 'anime'}{(tab === 'manga' ? $library.length : $animeLibrary.length) !== 1 ? 's' : ''}
    </span>
    <div class="header-actions">
      <button class="action-btn" onclick={importLibrary} title="Importar biblioteca desde JSON">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
        Importar
      </button>
      <button class="action-btn" onclick={exportLibrary} title="Exportar biblioteca a JSON">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="7 10 12 15 17 10"/>
          <line x1="12" y1="15" x2="12" y2="3"/>
        </svg>
        Exportar
      </button>
    </div>
  </div>

  {#if feedback}
    <div class="feedback" class:error={feedback.startsWith('Error')}>{feedback}</div>
  {/if}

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
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .library-title { font-size: 16px; font-weight: 800; color: var(--text); }
  .library-count { font-size: 11px; color: var(--text-muted); font-weight: 600; }

  .header-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }

  .action-btn {
    display: flex; align-items: center; gap: 5px;
    padding: 5px 10px;
    background: var(--bg-card, rgba(255,255,255,0.05));
    border: 1px solid var(--outline-dim);
    border-radius: 7px;
    color: var(--text-muted);
    font-size: 11px; font-weight: 600;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
  }
  .action-btn svg { width: 13px; height: 13px; flex-shrink: 0; }
  .action-btn:hover {
    color: var(--primary, #f59e0b);
    border-color: var(--primary, #f59e0b);
    background: rgba(245,158,11,0.08);
  }

  .feedback {
    font-size: 11px; font-weight: 600;
    padding: 7px 12px; border-radius: 7px;
    background: rgba(52,211,153,0.12);
    color: #34d399;
    border: 1px solid rgba(52,211,153,0.25);
    flex-shrink: 0;
    animation: fadein 0.2s ease;
  }
  .feedback.error {
    background: rgba(248,113,113,0.12);
    color: #f87171;
    border-color: rgba(248,113,113,0.25);
  }
  @keyframes fadein { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: none; } }

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
