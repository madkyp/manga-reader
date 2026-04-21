<script>
  import { onMount, onDestroy } from 'svelte';
  import { readerPages, currentChapter, loadingReader, currentManga, selectedManga, goBack, openChapter } from '../stores/manga.js';

  // Lista de capítulos del manga actual (orden original: [0]=más nuevo)
  let chapters = $derived($currentManga?.chapters ?? []);

  // Índice del capítulo actual en la lista
  let currentIdx = $derived(
    chapters.findIndex(c => c.id === $currentChapter?.id)
  );

  // En orden de array: índice menor = capítulo más nuevo
  // Anterior en lectura (número menor) = índice mayor
  // Siguiente en lectura (número mayor) = índice menor
  let prevChapter = $derived(currentIdx < chapters.length - 1 ? chapters[currentIdx + 1] : null);
  let nextChapter = $derived(currentIdx > 0 ? chapters[currentIdx - 1] : null);

  function getMangaId() {
    return $selectedManga?.id || $currentManga?.id || null;
  }

  function goToPrev() {
    if (prevChapter && !$loadingReader) openChapter(prevChapter, getMangaId());
  }
  function goToNext() {
    if (nextChapter && !$loadingReader) openChapter(nextChapter, getMangaId());
  }

  function handleKey(e) {
    if (e.key === 'ArrowLeft')  goToPrev();
    if (e.key === 'ArrowRight') goToNext();
  }

  onMount(() => window.addEventListener('keydown', handleKey));
  onDestroy(() => window.removeEventListener('keydown', handleKey));
</script>

<div class="reader">
  <!-- Barra superior -->
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>

    <span class="chapter-label">
      {$currentManga?.title || ''} — {$currentChapter?.title || ''}
    </span>

    <div class="chapter-nav">
      <button
        class="nav-btn"
        disabled={!prevChapter || $loadingReader}
        onclick={goToPrev}
        title="Capítulo anterior (←)"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
        Anterior
      </button>
      <button
        class="nav-btn"
        disabled={!nextChapter || $loadingReader}
        onclick={goToNext}
        title="Capítulo siguiente (→)"
      >
        Siguiente
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
      </button>
    </div>
  </div>

  <!-- Área de páginas -->
  <div class="pages">
    {#if $loadingReader}
      <p class="status-text">Cargando capítulo...</p>

    {:else if $readerPages.length === 0}
      <p class="status-text">No se encontraron páginas.</p>

    {:else}
      {#each $readerPages as page, i (page.url)}
        <img
          class="page-img"
          src={page.url}
          alt="Página {i + 1}"
          loading="lazy"
        />
      {/each}
    {/if}
  </div>
</div>

<style>
  .reader {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #000;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
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
    white-space: nowrap;
    flex-shrink: 0;
  }
  .back-btn:hover { background: var(--bg-card); }

  .chapter-label {
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .chapter-nav {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 5px;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
    white-space: nowrap;
  }
  .nav-btn svg { width: 12px; height: 12px; flex-shrink: 0; }
  .nav-btn:hover:not(:disabled) {
    background: var(--bg-card-high);
    border-color: var(--primary);
    color: var(--primary);
  }
  .nav-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .pages {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .page-img {
    width: 100%;
    max-width: 800px;
    display: block;
    object-fit: contain;
  }

  .status-text {
    color: var(--text-muted);
    font-size: 14px;
    padding: 40px 20px;
    text-align: center;
  }
</style>
