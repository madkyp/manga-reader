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
  <!-- Columna izquierda: logo encima + sidebar debajo -->
  {#if $currentView !== 'reader'}
    <div class="left-panel">
      <div class="logo-block">
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

  .left-panel {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .logo-block {
    background: var(--bg-low);
    border-right: 1px solid var(--outline-dim);
    border-bottom: 1px solid var(--outline-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 10px 8px;
    flex-shrink: 0;
  }

  .logo-block img {
    width: 48px;
    height: 48px;
    object-fit: contain;
  }

  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }


</style>
