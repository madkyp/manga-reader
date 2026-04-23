<script>
  // Componente raíz — layout principal con sidebar izquierda y área de contenido
  import Sidebar    from './components/Sidebar.svelte';
  import HomeView   from './components/HomeView.svelte';
  import BrowseView from './components/BrowseView.svelte';
  import DetailView from './components/DetailView.svelte';
  import ReaderView from './components/ReaderView.svelte';
  import SettingsView from './components/SettingsView.svelte';
  import LibraryView        from './components/LibraryView.svelte';
  import NotificationsView  from './components/NotificationsView.svelte';
  import AnimeView          from './components/AnimeView.svelte';
  import { onMount } from 'svelte';
  import { currentView, checkNotifications } from './stores/manga.js';
  import { checkAnimeNotifications } from './stores/anime.js';

  const INTERVAL_MS = 5 * 60 * 1000;

  onMount(() => {
    checkNotifications();
    checkAnimeNotifications();
    const timer = setInterval(() => { checkNotifications(); checkAnimeNotifications(); }, INTERVAL_MS);
    return () => clearInterval(timer);
  });
</script>

<div class="app">
  <!-- Columna izquierda: logo + navegación (oculta en el lector) -->
  {#if $currentView !== 'reader'}
    <div class="left-col">
      <div class="app-logo">
        <img src="/logo.png" alt="The Foundry" />
      </div>
      <Sidebar />
    </div>
  {/if}

  <!-- Área de contenido principal -->
  <main class="content">
    {#if $currentView === 'home'}
      <HomeView />
    {:else if $currentView === 'browse'}
      <BrowseView />
    {:else if $currentView === 'detail'}
      <DetailView />
    {:else if $currentView === 'reader'}
      <ReaderView />
    {:else if $currentView === 'settings'}
      <SettingsView />
    {:else if $currentView === 'library'}
      <LibraryView />
    {:else if $currentView === 'notifications'}
      <NotificationsView />
    {:else if $currentView === 'anime'}
      <AnimeView />
    {/if}
  </main>
</div>

<style>
  .app {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: row;
  }

  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .left-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--bg-low);
    border-right: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .app-logo {
    padding: 12px 8px 4px;
    flex-shrink: 0;
  }

  .app-logo img {
    width: 120px;
    height: 120px;
    object-fit: contain;
    display: block;
  }

</style>
