<script>
  // Sidebar de navegación izquierda — iconos para cambiar de sección
  // Solo cambia entre secciones principales (home, browse)
  // Las vistas detail y reader tienen su propio botón "volver"
  import { currentView, unreadCount } from '../stores/manga.js';

  const mainItems = [
    { id: 'home',          label: 'Inicio'         },
    { id: 'browse',        label: 'Manwhas'        },
    { id: 'anime',         label: 'Anime'          },
    { id: 'library',       label: 'Biblioteca'     },
    { id: 'notifications', label: 'Notificaciones' },
  ];

  function navigate(id, disabled) {
    if (disabled) return;
    currentView.set(id);
  }

  let activeSection = $derived(
    $currentView === 'detail' || $currentView === 'reader' ? 'browse' :
    $currentView === 'anime' ? 'anime' :
    $currentView === 'home' ? 'home' :
    $currentView === 'library' ? 'library' :
    $currentView === 'notifications' ? 'notifications' : $currentView
  );
</script>

<nav class="sidebar">
  <!-- Items de navegación principales -->
  <div class="nav-items">
    {#each mainItems as item}
      <button
        class="nav-btn"
        class:active={activeSection === item.id}
        class:disabled={item.disabled}
        title={item.disabled ? item.label + ' (próximamente)' : item.label}
        onclick={() => navigate(item.id, item.disabled)}
      >
        <span class="nav-icon">
          {#if item.id === 'home'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
          {:else if item.id === 'browse'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
          {:else if item.id === 'anime'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
          {:else if item.id === 'library'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
          {:else if item.id === 'notifications'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
          {/if}
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.id === 'notifications' && $unreadCount > 0}
          <span class="notif-badge">{$unreadCount}</span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- Opciones al fondo -->
  <div class="nav-bottom">
    <button
      class="nav-btn"
      class:active={activeSection === 'settings'}
      title="Opciones"
      onclick={() => navigate('settings', false)}
    >
      <span class="nav-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
      </span>
      <span class="nav-label">Opciones</span>
    </button>
  </div>
</nav>

<style>
  .sidebar {
    width: 64px;
    background: transparent;
    border-right: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 0;
    flex-shrink: 0;
    gap: 4px;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 0 8px;
    flex: 1;
  }

  .nav-bottom {
    width: 100%;
    padding: 0 8px 4px;
    flex-shrink: 0;
    border-top: 1px solid var(--outline-dim);
    padding-top: 6px;
    margin-top: 4px;
  }

  .nav-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 8px 4px;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: 8px;
    transition: background 0.15s, color 0.15s;
    width: 100%;
  }
  .nav-btn:hover:not(.disabled) { background: var(--bg-card); color: var(--text); }
  .nav-btn.active { background: var(--bg-card-high); color: var(--primary); }
  .nav-btn.disabled { opacity: 0.35; cursor: default; }

  .nav-icon { width: 20px; height: 20px; display: flex; align-items: center; justify-content: center; }
  .nav-icon svg { width: 18px; height: 18px; }
  .nav-label { font-size: 9px; font-weight: 600; letter-spacing: 0.03em; }

  .notif-badge {
    position: absolute;
    top: 6px;
    right: 6px;
    background: var(--primary, #f59e0b);
    color: #000;
    font-size: 9px;
    font-weight: 800;
    border-radius: 99px;
    padding: 1px 4px;
    line-height: 1.4;
    pointer-events: none;
  }

  .nav-btn { position: relative; }
</style>
