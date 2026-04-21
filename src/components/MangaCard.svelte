<script>
  // latest=true → muestra badge inferior (pestaña Últimos)
  // openchapter → función(chapterId, chapterTitle) para navegación directa al cap
  const { item, onclick, latest = false, openchapter = null } = $props();

  function statusColor(status) {
    const s = (status || '').toLowerCase();
    if (s.includes('activo'))     return 'var(--green)';
    if (s.includes('pausado'))    return 'var(--orange)';
    if (s.includes('cancelado'))  return 'var(--red)';
    return 'var(--gray)';
  }

  function statusLabel(status) {
    const s = (status || '').toLowerCase();
    if (s.includes('activo'))     return 'Activo';
    if (s.includes('pausado'))    return 'Pausa';
    if (s.includes('cancelado'))  return 'Cancel.';
    if (s.includes('finalizado')) return 'Fin.';
    return status || '';
  }

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

  // "Capítulo 123" / "Cap. 123" / "123.5" → "Cap 123" / "Cap 123.5"
  function shortChapter(str) {
    if (!str) return 'Nuevo';
    const m = str.match(/(\d+(?:[.,]\d+)?)/);
    return m ? `Cap ${m[1].replace(',', '.')}` : str.slice(0, 7);
  }

  let ago1 = $derived(isoAgo(item.date));
  let ago2 = $derived(isoAgo(item.date2));

  function handleCh1(e) {
    e.stopPropagation();
    openchapter(item.chapter_id || null, item.chapter || 'Nuevo');
  }
  function handleCh2(e) {
    e.stopPropagation();
    openchapter(item.chapter2_id || null, item.chapter2);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="card" onclick={() => onclick(item)}>
  <div class="cover-wrap">
    <img
      class="cover"
      src={item.image}
      alt={item.title}
      loading="lazy"
      onerror={(e) => e.target.src='https://picsum.photos/120/170'}
    />

    <!-- Badge estado — esquina superior izquierda -->
    {#if item.status}
      <div class="badge badge-status" style="color: {statusColor(item.status)}">
        <span class="dot" style="background: {statusColor(item.status)}"></span>
        {statusLabel(item.status)}
      </div>
    {/if}

    <!-- Badge tipo — esquina superior derecha -->
    {#if item.type}
      <div class="badge badge-type">{item.type.toUpperCase()}</div>
    {/if}

    <!-- Franja inferior — botones de capítulo cuando openchapter está disponible -->
    {#if latest || item.chapter}
      <div class="badge-bottom">

        {#if openchapter}
          <!-- Modo botones: clickable chips que llevan directamente al capítulo -->
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="ch-btn" onclick={handleCh1}>
            <span class="ch-btn-label">{shortChapter(item.chapter)}</span>
            <span class="ch-btn-time">
              <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
                <circle cx="6" cy="6" r="4.5"/>
                <path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/>
              </svg>
              {ago1 || 'Ahora'}
            </span>
          </div>

          {#if item.chapter2 || ago2}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="ch-btn ch-btn-prev" onclick={handleCh2}>
              {#if item.chapter2}
                <span class="ch-btn-label ch-btn-label-prev">{shortChapter(item.chapter2)}</span>
              {/if}
              {#if ago2}
                <span class="ch-btn-time ch-btn-time-prev">{ago2}</span>
              {/if}
            </div>
          {/if}

        {:else}
          <!-- Modo solo visualización (browse sin openchapter) -->
          <div class="cap-slot">
            <span class="cap-name">{item.chapter || 'Nuevo'}</span>
            {#if item.chapter}
              <span class="cap-time">
                <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
                  <circle cx="6" cy="6" r="4.5"/>
                  <path d="M6 3.5v2.8l1.6 1.6" stroke-linecap="round"/>
                </svg>
                {ago1 || 'Ahora'}
              </span>
            {/if}
          </div>

          {#if item.chapter2}
            <div class="cap-slot cap-slot-prev">
              <span class="cap-name cap-name-prev">{item.chapter2}</span>
              {#if ago2}
                <span class="cap-time cap-time-prev">{ago2}</span>
              {/if}
            </div>
          {/if}
        {/if}

      </div>
    {/if}
  </div>

  <p class="title" title={item.title}>{item.title}</p>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    cursor: pointer;
    width: 170px;
  }
  .card:hover .cover { opacity: 0.85; transform: scale(1.02); }

  .cover-wrap {
    position: relative;
    width: 170px;
    height: 240px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-card);
  }

  .cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: opacity 0.15s, transform 0.15s;
  }

  /* Badges superiores */
  .badge {
    position: absolute;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 5px;
    border-radius: 3px;
    background: rgba(2,4,32,0.82);
    backdrop-filter: blur(2px);
    line-height: 1.4;
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .badge-status { top: 5px; left: 5px; }
  .badge-type   { top: 5px; right: 5px; color: var(--secondary); }

  .dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }

  /* ── Franja inferior ── */
  .badge-bottom {
    position: absolute;
    bottom: 0; left: 0; right: 0;
    background: linear-gradient(to top, rgba(0,0,0,0.92) 0%, rgba(0,0,0,0.6) 55%, transparent 100%);
    padding: 22px 5px 5px;
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: flex-end;
    gap: 3px;
  }

  /* ── Modo botones (openchapter activo) ── */
  .ch-btn {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    max-width: 55%;
    background: rgba(0,0,0,0.72);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 5px;
    padding: 3px 5px;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
  }
  .ch-btn:hover {
    background: rgba(232,121,249,0.18);
    border-color: rgba(232,121,249,0.45);
  }

  .ch-btn-prev {
    align-items: flex-end;
    max-width: 42%;
    background: rgba(0,0,0,0.55);
    border-color: rgba(255,255,255,0.07);
  }
  .ch-btn-prev:hover {
    background: rgba(232,121,249,0.1);
    border-color: rgba(232,121,249,0.3);
  }

  .ch-btn-label {
    font-size: 10px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
  }

  .ch-btn-label-prev {
    font-size: 9px;
    font-weight: 600;
    color: rgba(255,255,255,0.55);
    text-align: right;
  }

  .ch-btn-time {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 9.5px;
    color: #e879f9;
    font-weight: 600;
    white-space: nowrap;
  }
  .ch-btn-time svg { width: 9px; height: 9px; flex-shrink: 0; stroke: #e879f9; }

  .ch-btn-time-prev {
    color: rgba(232,121,249,0.5);
    justify-content: flex-end;
  }
  .ch-btn-time-prev svg { stroke: rgba(232,121,249,0.5); }

  /* ── Modo visualización (sin openchapter) ── */
  .cap-slot {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    max-width: 48%;
  }

  .cap-slot-prev { align-items: flex-end; }

  .cap-name {
    font-size: 10.5px;
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

  .cap-name-prev {
    font-size: 9.5px;
    font-weight: 600;
    color: rgba(255,255,255,0.5);
    background: rgba(255,255,255,0.06);
  }

  .cap-time {
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 9.5px;
    color: #e879f9;
    white-space: nowrap;
    font-weight: 600;
  }
  .cap-time svg { width: 9px; height: 9px; flex-shrink: 0; stroke: #e879f9; }

  .cap-time-prev {
    color: rgba(232,121,249,0.45);
    justify-content: flex-end;
  }
  .cap-time-prev svg { stroke: rgba(232,121,249,0.45); }

  .title {
    font-size: 12px;
    color: var(--text);
    line-height: 1.3;
    max-width: 170px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
