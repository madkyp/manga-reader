<script>
  import { onMount, onDestroy } from 'svelte';
  import { animeLibrary, toggleAnimeLibrary, pendingAnime } from '../stores/anime.js';
  import TorrentModal from './TorrentModal.svelte';

  async function tauri(cmd, args) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke(cmd, args);
  }

  // ── Estado global de la vista ─────────────────────────────────────────────
  let subView  = $state('browse'); // 'browse' | 'detail' | 'player'
  let loading  = $state(false);
  let error    = $state('');
  let items    = $state([]);
  let detail   = $state(null);
  let streams  = $state([]);
  let episode  = $state(null);

  // ── Fuente activa ─────────────────────────────────────────────────────────
  let activeSource = $state('flv'); // 'flv' | 'kitsu'

  // ── Browse ────────────────────────────────────────────────────────────────
  let activeTab   = $state('latest');
  let searchQuery = $state('');
  let searchTimer = null;
  let page        = $state(1);
  let hasMore     = $state(false);
  let loadingMore = $state(false);

  // ── FLV browse ────────────────────────────────────────────────────────────
  async function loadLatest() {
    loading = true; error = ''; page = 1; hasMore = false;
    try {
      const json = await tauri('animeflv_latest');
      items = JSON.parse(json);
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  async function loadPaged(order, p = 1, append = false) {
    if (append) loadingMore = true; else { loading = true; items = []; }
    error = '';
    try {
      const json = await tauri('animeflv_browse', { page: p, order });
      const next = JSON.parse(json);
      items = append ? [...items, ...next] : next;
      hasMore = next.length >= 24;
      page = p;
    } catch(e) { error = String(e); }
    finally { loading = false; loadingMore = false; }
  }

  async function doSearchFlv(query) {
    loading = true; error = ''; hasMore = false;
    try {
      const json = await tauri('animeflv_search', { query });
      items = JSON.parse(json);
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  async function switchTab(tab) {
    activeTab = tab; searchQuery = ''; page = 1; hasMore = false;
    if (activeSource === 'kitsu') { await loadKitsuTab(tab); return; }
    if (tab === 'latest')         await loadLatest();
    else if (tab === 'recientes') await loadPaged('updated', 1);
    else if (tab === 'explorar')  await loadPaged('title', 1);
  }

  async function loadMore() {
    if (activeSource === 'kitsu') { await loadKitsuMore(); return; }
    const order = activeTab === 'explorar' ? 'title' : 'updated';
    await loadPaged(order, page + 1, true);
  }

  // ── Kitsu browse ──────────────────────────────────────────────────────────
  async function loadKitsuTab(tab) {
    loading = true; error = ''; items = []; page = 1; hasMore = false;
    try {
      let json;
      if (tab === 'latest') {
        json = await tauri('kitsu_trending');
      } else {
        const sort = tab === 'explorar' ? 'title' : 'rating';
        json = await tauri('kitsu_browse', { sort, page: 1 });
      }
      const r = JSON.parse(json);
      items = r.items ?? r;
      hasMore = r.has_more ?? false;
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  async function loadKitsuMore() {
    if (loadingMore) return;
    loadingMore = true;
    try {
      const sort = activeTab === 'explorar' ? 'title' : 'rating';
      const json = await tauri('kitsu_browse', { sort, page: page + 1 });
      const r = JSON.parse(json);
      items = [...items, ...(r.items ?? [])];
      hasMore = r.has_more ?? false;
      page = page + 1;
    } catch(e) { error = String(e); }
    finally { loadingMore = false; }
  }

  async function doSearchKitsu(query) {
    loading = true; error = ''; hasMore = false;
    try {
      const json = await tauri('kitsu_search', { query });
      const r = JSON.parse(json);
      items = r.items ?? [];
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  function handleSearch() {
    clearTimeout(searchTimer);
    if (!searchQuery.trim()) { switchTab(activeTab); return; }
    const q = searchQuery.trim();
    searchTimer = setTimeout(() => {
      if (activeSource === 'kitsu') doSearchKitsu(q);
      else doSearchFlv(q);
    }, 500);
  }

  function clearSearch() { searchQuery = ''; switchTab(activeTab); }

  async function switchSource(src) {
    activeSource = src;
    searchQuery = ''; page = 1; hasMore = false; items = [];
    activeTab = 'latest';
    await switchTab('latest');
  }

  onMount(() => switchTab('latest'));
  onDestroy(() => clearInterval(torrentPollId));

  $effect(() => {
    const p = $pendingAnime;
    if (p) { pendingAnime.set(null); openAnime(p); }
  });

  // ── Detail ────────────────────────────────────────────────────────────────
  let epDates   = $state({});
  let watchedSet = $state(new Set());

  function loadWatched(animeId) {
    try {
      const raw = localStorage.getItem(`foundry_watched_${animeId}`);
      return raw ? new Set(JSON.parse(raw)) : new Set();
    } catch { return new Set(); }
  }
  function saveWatched(animeId, ids) {
    try { localStorage.setItem(`foundry_watched_${animeId}`, JSON.stringify([...ids])); } catch {}
  }
  function markWatched(epId) {
    const id = detail?.id; if (!id) return;
    watchedSet = new Set([...watchedSet, epId]);
    saveWatched(id, watchedSet);
  }
  function markAllWatched() {
    const id = detail?.id; if (!id) return;
    watchedSet = new Set((detail?.episodes ?? []).map(e => e.id));
    saveWatched(id, watchedSet);
  }
  let allWatched = $derived(
    (detail?.episodes?.length ?? 0) > 0 &&
    (detail?.episodes ?? []).every(e => watchedSet.has(e.id))
  );

  // fuente guardada al abrir detalle (para saber si los eps son kitsu o flv)
  let detailSource = $state('flv');
  let kitsuEpsPage = $state(1);
  let kitsuEpsMore = $state(false);

  async function openAnime(item) {
    detailSource = activeSource;
    if (activeSource === 'kitsu') { await openKitsuAnime(item); return; }

    subView = 'detail';
    detail = { ...item, episodes: [], genres: [], synopsis: '', status: '' };
    epDates = {}; watchedSet = loadWatched(item.id);
    loading = true; error = '';
    try {
      const [detailJson, datesJson] = await Promise.allSettled([
        tauri('animeflv_detail', { slug: item.id }),
        tauri('anilist_episode_dates', { title: item.title }),
      ]);
      if (detailJson.status === 'fulfilled') detail = JSON.parse(detailJson.value);
      if (datesJson.status === 'fulfilled') epDates = JSON.parse(datesJson.value);
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  async function openKitsuAnime(item) {
    subView = 'detail';
    detail = { ...item, episodes: [], genres: item.genres ?? [], synopsis: item.synopsis ?? '', status: item.status ?? '' };
    epDates = {}; watchedSet = loadWatched(item.id);
    kitsuEpsPage = 1; kitsuEpsMore = false;
    loading = true; error = '';
    try {
      const [detailJson, epsJson, datesJson] = await Promise.allSettled([
        tauri('kitsu_detail',   { id: item.id }),
        tauri('kitsu_episodes', { animeId: item.id }),
        tauri('anilist_episode_dates', { title: item.title }),
      ]);

      // Construir el objeto final en una variable local, luego asignar de una vez
      let base = detailJson.status === 'fulfilled'
        ? JSON.parse(detailJson.value)
        : { ...item };

      // AniList dates (más completas que Kitsu para episodios recientes)
      const allDates = datesJson.status === 'fulfilled' ? JSON.parse(datesJson.value) : {};
      if (datesJson.status === 'fulfilled') epDates = allDates;

      let episodes = [];
      if (epsJson.status === 'fulfilled') {
        const r = JSON.parse(epsJson.value);
        const today = new Date().toISOString().split('T')[0];
        const rawEps = (r.episodes ?? []).map(kitsuEpToUnified);

        // Máximo desde fechas de Kitsu
        const maxKitsu = rawEps
          .filter(ep => ep.airdate && ep.airdate <= today)
          .reduce((max, ep) => Math.max(max, ep.number), 0);

        // Máximo desde fechas de AniList (suelen estar más al día)
        const maxAniList = Object.keys(allDates).reduce((max, k) => {
          const n = parseInt(k); return isNaN(n) ? max : Math.max(max, n);
        }, 0);

        const maxEpCount = base.episode_count ?? 0;
        const maxAired = Math.max(maxKitsu, maxAniList, maxEpCount);
        const cap = maxAired > 0 ? maxAired + 20 : Infinity;
        episodes = rawEps
          .filter(ep => ep.number <= cap)
          .sort((a, b) => b.number - a.number);
        kitsuEpsMore = false;
      } else {
        error = 'Episodios: ' + String(epsJson.reason);
      }

      detail = { ...base, episodes };
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  async function loadMoreKitsuEps() {
    if (!detail) return;
    const nextPage = kitsuEpsPage + 1;
    try {
      const json = await tauri('kitsu_episodes', { animeId: detail.id, page: nextPage });
      const r = JSON.parse(json);
      const newEps = (r.episodes ?? []).map(kitsuEpToUnified);
      detail = { ...detail, episodes: [...detail.episodes, ...newEps].sort((a, b) => b.number - a.number) };
      kitsuEpsPage = nextPage;
      kitsuEpsMore = r.has_more ?? false;
    } catch(e) { error = String(e); }
  }

  // Normaliza episodio Kitsu al formato unificado {id, number, title}
  function kitsuEpToUnified(ep) {
    return { id: `k-${ep.id}`, number: ep.number, title: ep.title, airdate: ep.airdate, thumbnail: ep.thumbnail, length_min: ep.length_min };
  }

  // ── Player ────────────────────────────────────────────────────────────────
  let selectedServer = $state(null);

  async function openEpisode(ep) {
    // Kitsu: no tiene streams propios → abrir TorrentModal directamente
    if (detailSource === 'kitsu') {
      markWatched(ep.id);
      torrentModal = { animeTitle: detail?.title ?? '', episodeNumber: ep.number };
      return;
    }

    markWatched(ep.id);
    episode = ep; streams = []; subView = 'player';
    loading = true; error = '';
    try {
      const json = await tauri('animeflv_streams', { episodeId: ep.id });
      streams = JSON.parse(json);
      if (streams.length > 0) {
        const ext = streams.find(s => EXTRACTABLE.some(k => s.url.toLowerCase().includes(k)));
        await selectServer(ext || streams[0]);
      }
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }

  function goBack() {
    if (subView === 'player') subView = 'detail';
    else subView = 'browse';
  }

  let directUrl = $state(null);
  let extracting = $state(false);
  let extractError = $state('');

  // ── Torrent ───────────────────────────────────────────────────────────────
  let torrentModal   = $state(null);
  let torrentPlayer  = $state(null);
  let torrentPollId  = null;
  let showSubMenu    = $state(false);
  let activeSub      = $state(null);
  let subLoadError   = $state('');

  async function handleTorrentSelect({ torrent, file }) {
    torrentModal = null;

    const candidates = [torrent.magnet, torrent.torrent].filter(Boolean);
    if (candidates.length === 0) {
      error = 'Este torrent no tiene magnet ni URL .torrent válida.';
      return;
    }

    // Mostrar reproductor en loading inmediatamente
    torrentPlayer = {
      torrentId: null,
      fileUrl: null,
      fileName: file?.name || 'Conectando al torrent…',
      status: null,
      subs: [],
      activeSub: null,
      videoError: '',
      needsExternal: false,
      loading: true,
    };
    transmuxOffset = 0;
    streamBaseline = 0;
    needsBaseline  = true;
    videoCurrent = 0;

    let result = null;
    let lastErr = '';
    for (const url of candidates) {
      try {
        console.log('[torrent] intentando:', url.slice(0, 80));
        result = await tauri('torrent_start', { url });
        break;
      } catch(e) {
        lastErr = String(e);
        console.warn('[torrent] falló:', lastErr);
      }
    }
    if (!result) {
      torrentPlayer = { ...torrentPlayer, loading: false, videoError: 'Error iniciando torrent: ' + lastErr };
      return;
    }

    try {

      // `file` viene de nyaa_torrent_info (no tiene stream_url), hay que matchear
      // contra result.files (que sí lo tiene) por nombre.
      let fileEntry = null;
      if (file && result.files?.length > 0) {
        fileEntry = result.files.find(f => f.name === file.name)
                 ?? result.files.find(f => f.rel_path === file.path);
      }
      if (!fileEntry && result.files?.length > 0) {
        // Fallback: primer video, o el más grande si no hay flag is_video
        fileEntry = result.files.find(f => f.is_video)
                 ?? [...result.files].sort((a, b) => b.size - a.size)[0];
      }

      const fileUrl = fileEntry?.stream_url ?? null;
      if (!fileUrl) {
        error = 'No se pudo obtener stream del torrent (sin archivos jugables).';
        return;
      }

      const ext = (fileEntry.name.split('.').pop() || '').toLowerCase();
      const needsTransmux = ['mkv', 'avi', 'mov'].includes(ext);

      // Subs externos (.srt/.ass/.vtt sueltos en el torrent)
      const subExts = ['srt', 'vtt', 'ass', 'ssa'];
      const subFiles = (result.files ?? []).filter(f => {
        const e = (f.name.split('.').pop() || '').toLowerCase();
        return subExts.includes(e);
      }).map(f => ({
        url: f.stream_url,
        name: f.name,
        lang: detectSubLang(f.name),
        embedded: false,
      }));

      // Si es MKV/AVI/MOV → usar transmux (ffmpeg) + extraer subs embebidos
      let playUrl = fileUrl;
      let allSubs = subFiles;
      if (needsTransmux) {
        try {
          const t = await tauri('torrent_transmux', {
            torrentId: result.torrent_id,
            fileIdx: fileEntry.index,
          });
          playUrl = t.video_url;
          if (t.duration_secs > 0) videoDuration = t.duration_secs;
          const embedded = (t.subs ?? []).map(s => ({
            url:    s.url,
            name:   s.title || s.lang || `Sub ${s.index + 1}`,
            lang:   (s.lang || '').toUpperCase(),
            codec:  s.codec || '',
            embedded: true,
          }));
          allSubs = [...embedded, ...subFiles];
        } catch(e) {
          torrentPlayer = { ...torrentPlayer, loading: false, videoError: `ffmpeg falló (¿instalado?): ${e}` };
          return;
        }
      }

      // Calcular aviso de bitmap ANTES de sobrescribir torrentPlayer
      let initialVideoError = '';
      if (needsTransmux && allSubs.length === 0 && subFiles.length === 0) {
        // No hay subs de texto — se mostrará el botón "Sub" para re-detectar
      }

      torrentPlayer = {
        torrentId:      result.torrent_id,
        fileUrl:        playUrl,
        fileName:       fileEntry?.name || result.name,
        status:         null,
        subs:           allSubs,
        activeSub:      null,
        videoError:     initialVideoError,
        needsExternal:  false,
        loading:        false,
        _fileIdx:       fileEntry?.index ?? 0,
        _subFiles:      subFiles,
        _needsTransmux: needsTransmux,
        _transmuxBase:  needsTransmux ? playUrl : null,
      };

      // Arrancar polling de progreso
      clearInterval(torrentPollId);
      torrentPollId = setInterval(pollTorrentStatus, 2000);

    } catch(e) {
      error = 'Error iniciando torrent: ' + String(e);
    }
  }

  async function pollTorrentStatus() {
    if (!torrentPlayer) { clearInterval(torrentPollId); return; }
    try {
      const s = await tauri('torrent_status', { torrentId: torrentPlayer.torrentId });
      torrentPlayer = { ...torrentPlayer, status: s };
      if (s.finished) clearInterval(torrentPollId);
    } catch { clearInterval(torrentPollId); }
  }

  function closeTorrentPlayer() {
    clearInterval(torrentPollId);
    torrentPlayer = null;
    activeSub = null;
    transmuxOffset = 0;
    streamBaseline = 0;
    needsBaseline  = false;
    videoCurrent = 0;
  }

  function detectSubLang(name) {
    const n = name.toLowerCase();
    if (/\b(spa|esp|es|spanish|español|latino|lat)\b/.test(n)) return 'ES';
    if (/\b(eng|en|english)\b/.test(n))                       return 'EN';
    if (/\b(jpn|ja|japanese)\b/.test(n))                      return 'JP';
    return '';
  }

  function selectSub(sub) {
    activeSub = sub;
    if (torrentPlayer) torrentPlayer = { ...torrentPlayer, activeSub: sub };
  }

  let probingSubs = $state(false);
  async function probeSubs() {
    if (!torrentPlayer?.torrentId) return;
    probingSubs = true;
    try {
      const fileIdx = torrentPlayer._fileIdx ?? 0;
      const [tracks, bitmapCount] = await tauri('torrent_probe_subs', {
        torrentId: torrentPlayer.torrentId,
        fileIdx,
      });
      const subs = tracks.map(s => ({
        url:      s.url,
        name:     s.title || s.lang || `Sub ${s.index + 1}`,
        lang:     (s.lang || '').toUpperCase(),
        codec:    s.codec || '',
        embedded: true,
      }));
      const allSubs = [...subs, ...(torrentPlayer._subFiles ?? [])];
      let videoError = '';
      if (allSubs.length === 0 && bitmapCount > 0) {
        videoError = `Subtítulos de imagen (PGS/DVDSUB × ${bitmapCount}) — no renderizables. Usa reproductor externo.`;
      } else if (allSubs.length === 0) {
        videoError = 'ffprobe no detectó pistas de subtítulos de texto. Si el archivo aún está descargando, prueba más tarde.';
      }
      torrentPlayer = { ...torrentPlayer, subs: allSubs, videoError };
    } catch(e) {
      torrentPlayer = { ...torrentPlayer, videoError: `Error al detectar subtítulos: ${e}` };
    } finally {
      probingSubs = false;
    }
  }

  // Subtítulos: parseo propio + overlay CSS (WebKit ignora <track> en Tauri)
  let subCues        = $state([]);
  let currentSubLine = $state('');
  let videoEl        = $state(null);

  // Controles de reproducción personalizados
  let videoPaused     = $state(true);
  let videoDuration   = $state(0);   // duración efectiva (ffprobe o videoEl.duration)
  let videoCurrent    = $state(0);
  let bufferedEnd     = $state(0);
  let showControls    = $state(true);
  let seekLoading     = $state(false);
  let hideTimer       = null;
  // Offset del transmux: tiempo absoluto del archivo donde empieza el stream actual.
  // Tras cada seek, ffmpeg arranca en el keyframe más cercano (no en el tiempo pedido);
  // este offset es el tiempo real del keyframe, que obtenemos con transmux_nearest_keyframe.
  let transmuxOffset  = $state(0);
  // Baseline de currentTime del stream tras cada seek. Normalmente WebKit arranca
  // en 0 para fragmented MP4, pero puede tener un sesgo — lo capturamos y restamos.
  let streamBaseline  = 0;
  let needsBaseline   = false;

  function revealControls() {
    showControls = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => { if (!videoPaused) showControls = false; }, 3000);
  }

  function togglePlay() {
    if (!videoEl) return;
    if (videoEl.paused) videoEl.play(); else videoEl.pause();
  }

  async function seekTo(e) {
    const dur = videoDuration;
    if (!videoEl || dur <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const pct  = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const targetTime = pct * dur;

    if (torrentPlayer?._transmuxBase) {
      seekLoading = true;
      videoCurrent = targetTime;
      // Preguntar a ffprobe cuál será el keyframe real donde arranque ffmpeg.
      // Así transmuxOffset = tiempo REAL del stream, no el pedido → subs alineados.
      let actualStart = targetTime;
      try {
        actualStart = await tauri('transmux_nearest_keyframe', {
          torrentId: torrentPlayer.torrentId,
          fileIdx:   torrentPlayer._fileIdx ?? 0,
          target:    targetTime,
        });
        if (!actualStart || actualStart <= 0) actualStart = targetTime;
      } catch {
        actualStart = targetTime;
      }
      transmuxOffset = actualStart;
      streamBaseline = 0;
      needsBaseline  = true;
      const newSrc = `${torrentPlayer._transmuxBase}?start=${actualStart.toFixed(6)}`;
      videoEl.src = newSrc;
      videoEl.load();
      videoEl.play().catch(() => {});
    } else {
      transmuxOffset = 0;
      streamBaseline = 0;
      needsBaseline  = false;
      videoEl.currentTime = targetTime;
    }
  }

  function onVideoPlay()  { videoPaused = false; revealControls(); }
  function onVideoPause() { videoPaused = true;  showControls = true; clearTimeout(hideTimer); }
  function onCanPlay()    { seekLoading = false; }

  function onDurationChange(e) {
    const d = e.currentTarget.duration;
    // Para transmux el pipe no reporta duración → usar la de ffprobe (ya en videoDuration)
    if (isFinite(d) && d > 0) videoDuration = d;
  }
  function onProgress(e) {
    const buf = e.currentTarget.buffered;
    if (buf.length > 0) bufferedEnd = buf.end(buf.length - 1);
  }

  function fmtTime(s) {
    if (!s || isNaN(s) || !isFinite(s)) return '0:00';
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2,'0')}:${String(sec).padStart(2,'0')}`
      : `${m}:${String(sec).padStart(2,'0')}`;
  }

  // Depende solo de activeSub, NO de torrentPlayer → el polling cada 2s no resetea los subs
  $effect(() => {
    const sub = activeSub;
    subCues = []; currentSubLine = ''; subLoadError = '';
    if (!sub?.url) return;
    const url = sub.url;
    const codec = sub.codec || '';
    (async () => {
      try {
        const resp = await fetch(url);
        if (!resp.ok || resp.status === 204) {
          subLoadError = `No se pudo cargar la pista de subtítulos (HTTP ${resp.status})`;
          return;
        }
        const raw = await resp.text();
        if (!raw.trim()) {
          subLoadError = 'La pista de subtítulos está vacía (archivo incompleto o formato no soportado)';
          return;
        }
        // Para subs embebidos (url interna /transmux/.../sub/N) ya vienen como WebVTT.
        // Para subs externos, detectar por extensión o codec.
        const ext = url.split('?')[0].split('/').pop()?.split('.').pop()?.toLowerCase() ?? '';
        const isAss = codec === 'ass' || codec === 'ssa' || ext === 'ass' || ext === 'ssa';
        const isSrt = codec === 'subrip' || ext === 'srt';
        const vtt = isSrt ? srtToVtt(raw) : isAss ? assToVtt(raw) : raw;
        const cues = parseVttCues(vtt);
        if (cues.length === 0) {
          subLoadError = 'No se encontraron líneas de subtítulo (puede que el archivo esté incompleto)';
          return;
        }
        subCues = cues;
      } catch(e) {
        subLoadError = `Error al cargar subtítulos: ${String(e)}`;
        subCues = [];
      }
    })();
  });

  function parseVttCues(text) {
    const cues = [];
    // Normalizar saltos de línea (ffmpeg a veces usa \r\n)
    const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
    for (const block of normalized.split(/\n\n+/)) {
      const lines = block.trim().split('\n');
      const ti = lines.findIndex(l => l.includes('-->'));
      if (ti < 0) continue;
      const parts = lines[ti].split('-->');
      if (parts.length < 2) continue;
      const start = vttTimeToSec(parts[0].trim().split(/\s/)[0]);
      const end   = vttTimeToSec(parts[1].trim().split(/\s/)[0]);
      const body  = lines.slice(ti + 1).join('\n').replace(/<[^>]+>/g, '').trim();
      if (body && !isNaN(start) && !isNaN(end)) cues.push({ start, end, text: body });
    }
    return cues;
  }

  function vttTimeToSec(t) {
    const p = t.split(':').map(Number);
    return p.length === 3 ? p[0]*3600 + p[1]*60 + p[2] : p[0]*60 + p[1];
  }

  function onTimeUpdate(e) {
    // realT = tiempo absoluto del archivo. El stream de ffmpeg empieza en 0 tras
    // cada seek; transmuxOffset es el tiempo real del keyframe donde arrancó.
    const ct = e.currentTarget.currentTime;
    if (needsBaseline) {
      // Fragmented MP4 normalmente arranca en 0 pero si WebKit reporta otro valor
      // inicial (ej. tfdt residual), lo capturamos para alinear sub/imagen.
      streamBaseline = ct;
      needsBaseline  = false;
    }
    const streamT = ct - streamBaseline;
    const realT = transmuxOffset + streamT;
    videoCurrent = realT;
    const cue = subCues.find(c => realT >= c.start && realT <= c.end);
    currentSubLine = cue ? cue.text : '';
  }

  function srtToVtt(s) {
    const body = s.replace(/\r/g, '')
      .replace(/(\d\d:\d\d:\d\d),(\d\d\d)/g, '$1.$2');
    return 'WEBVTT\n\n' + body;
  }

  function assToVtt(s) {
    const lines = s.split(/\r?\n/);
    const events = [];
    let format = null;
    let inEvents = false;
    for (const line of lines) {
      if (line.trim() === '[Events]') { inEvents = true; continue; }
      if (!inEvents) continue;
      if (line.startsWith('Format:')) {
        format = line.slice('Format:'.length).split(',').map(x => x.trim());
        continue;
      }
      if (!line.startsWith('Dialogue:') || !format) continue;
      // Dividir solo por las primeras N-1 comas (el campo Text puede tener comas)
      const raw   = line.slice('Dialogue:'.length);
      const nCols = format.length;
      const parts = [];
      let pos = 0;
      for (let c = 0; c < nCols - 1; c++) {
        const comma = raw.indexOf(',', pos);
        if (comma < 0) break;
        parts.push(raw.slice(pos, comma));
        pos = comma + 1;
      }
      parts.push(raw.slice(pos)); // resto = campo Text con comas incluidas

      const si = format.indexOf('Start');
      const ei = format.indexOf('End');
      const ti = format.indexOf('Text');
      if (si < 0 || ei < 0 || ti < 0 || ti >= parts.length) continue;

      const start = assTimeToVtt(parts[si].trim());
      const end   = assTimeToVtt(parts[ei].trim());
      const text  = parts[ti]
        .replace(/\{[^}]*\}/g, '')  // tags de estilo {\\an8} etc.
        .replace(/\\N/g, '\n')
        .replace(/\\n/gi, '\n')
        .trim();
      if (text) events.push(`${start} --> ${end}\n${text}`);
    }
    return 'WEBVTT\n\n' + events.join('\n\n');
  }

  function assTimeToVtt(t) {
    // ASS: H:MM:SS.cc  → VTT: HH:MM:SS.ccc
    const m = t.match(/(\d+):(\d+):(\d+)\.(\d+)/);
    if (!m) return '00:00:00.000';
    const h  = m[1].padStart(2, '0');
    const cs = m[4].padEnd(3, '0').slice(0, 3);
    return `${h}:${m[2]}:${m[3]}.${cs}`;
  }

  async function openInExternal() {
    if (!torrentPlayer?.fileUrl) return;
    try {
      const p = await tauri('torrent_open_external', { url: torrentPlayer.fileUrl });
      torrentPlayer = { ...torrentPlayer, videoError: `Abierto en ${p} — déjalo corriendo en segundo plano` };
    } catch(e) {
      torrentPlayer = { ...torrentPlayer, videoError: 'No se encontró reproductor externo (instala mpv o vlc)' };
    }
  }

  function onVideoError(e) {
    if (!torrentPlayer) return;
    const code = e.currentTarget.error?.code;
    const msg = {
      1: 'Carga abortada',
      2: 'Error de red',
      3: 'Error de decodificación (códec no soportado — p.ej. H.265/HEVC)',
      4: 'Formato no soportado por el navegador',
    }[code] || 'Error desconocido';
    torrentPlayer = { ...torrentPlayer, videoError: msg };
  }

  function formatBytes(b) {
    if (!b) return '0 B';
    if (b > 1e9) return (b / 1e9).toFixed(1) + ' GB';
    if (b > 1e6) return (b / 1e6).toFixed(0) + ' MB';
    return (b / 1e3).toFixed(0) + ' KB';
  }

  const EXTRACTABLE = ['streamtape'];

  async function selectServer(s) {
    selectedServer = s;
    directUrl = null;
    extractError = '';
    const lower = (s.url || '').toLowerCase();
    if (EXTRACTABLE.some(k => lower.includes(k))) {
      extracting = true;
      try {
        directUrl = await tauri('animeflv_extract', { url: s.url });
      } catch(e) {
        extractError = String(e);
        // Stape falló: auto-seleccionar el primer servidor no-Stape si existe
        const fallback = streams.find(x => !EXTRACTABLE.some(k => x.url.toLowerCase().includes(k)));
        if (fallback && fallback.url !== s.url) {
          selectedServer = fallback;
        }
      } finally {
        extracting = false;
      }
    }
  }

  const proxied = new WeakSet();
  async function onImgError(e, url) {
    const el = e.currentTarget;
    if (!url || proxied.has(el)) { el.style.visibility = 'hidden'; return; }
    proxied.add(el);
    try {
      const dataUri = await tauri('animeflv_image', { url });
      el.src = dataUri;
      el.style.visibility = 'visible';
    } catch {
      el.style.visibility = 'hidden';
    }
  }

  function typeColor(t) {
    const lower = (t || '').toLowerCase();
    if (lower === 'tv')    return '#6366f1';
    if (lower === 'movie') return '#f59e0b';
    if (lower === 'ova')   return '#10b981';
    if (lower === 'ona')   return '#0ea5e9';
    return '#8b5cf6';
  }
</script>

<!-- ══════════════════════════════════════════════════════════════════════════ -->
<!-- BROWSE                                                                     -->
<!-- ══════════════════════════════════════════════════════════════════════════ -->
{#if subView === 'browse'}
<div class="view">
  <div class="header">
    <div class="source-tabs">
      <button class="stab" class:active={activeSource === 'flv'} onclick={() => switchSource('flv')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>
        AnimeFLV
      </button>
      <button class="stab" class:active={activeSource === 'kitsu'} onclick={() => switchSource('kitsu')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/></svg>
        AnimeKitsu
        <span class="torrent-badge">Torrent</span>
      </button>
    </div>
    <div class="search-wrap">
      <svg class="search-ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input class="search-input" type="text" placeholder="Buscar anime..." bind:value={searchQuery} oninput={handleSearch} />
      {#if searchQuery}
        <button class="search-clear" onclick={clearSearch}>✕</button>
      {/if}
    </div>
  </div>

  <div class="browse-tabs">
    {#if activeSource === 'flv'}
      <button class="btab" class:active={activeTab === 'latest'}    onclick={() => switchTab('latest')}>Últimos</button>
      <button class="btab" class:active={activeTab === 'recientes'} onclick={() => switchTab('recientes')}>Recientes</button>
      <button class="btab" class:active={activeTab === 'explorar'}  onclick={() => switchTab('explorar')}>Explorar</button>
    {:else}
      <button class="btab" class:active={activeTab === 'latest'}    onclick={() => switchTab('latest')}>Trending</button>
      <button class="btab" class:active={activeTab === 'recientes'} onclick={() => switchTab('recientes')}>Más valorados</button>
      <button class="btab" class:active={activeTab === 'explorar'}  onclick={() => switchTab('explorar')}>Explorar A–Z</button>
    {/if}
  </div>

  <div class="scroll-area">
    {#if loading}
      <div class="status-center"><div class="spinner"></div><span>Cargando...</span></div>
    {:else if error}
      <div class="status-center error">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <span>{error}</span>
        <button class="retry-btn" onclick={() => switchTab(activeTab)}>↻ Reintentar</button>
      </div>
    {:else if items.length === 0}
      <div class="status-center">Sin resultados</div>
    {:else}
      {#if searchQuery}
        <div class="section-label">Resultados para "<strong>{searchQuery}</strong>"</div>
      {/if}
      <div class="cards-grid">
        {#each items as item, i (i + '-' + item.id + '-' + item.episode)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="acard" onclick={() => openAnime(item)}>
            <div class="acard-cover-wrap">
              <img class="acard-cover" src={item.image} alt={item.title} onerror={(e) => onImgError(e, item.image)} />
              <span class="acard-type" style="background:{typeColor(item.type ?? item.anime_type)}22; color:{typeColor(item.type ?? item.anime_type)};">
                {item.type ?? item.anime_type ?? 'TV'}
              </span>
              {#if item.episode}
                <div class="acard-ep-badge">{item.episode}</div>
              {/if}
            </div>
            <p class="acard-title" title={item.title}>{item.title}</p>
          </div>
        {/each}
      </div>
      {#if hasMore && !searchQuery}
        <div class="load-more-wrap">
          <button class="load-more-btn" onclick={loadMore} disabled={loadingMore}>
            {#if loadingMore}<div class="spinner-sm"></div> Cargando...{:else}Cargar más{/if}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

{:else if subView === 'detail'}
<div class="view">
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>
    <span class="topbar-title">{detail?.title ?? ''}</span>
  </div>
  <div class="scroll-area">
    {#if loading && !detail?.episodes?.length}
      <div class="status-center"><div class="spinner"></div><span>Cargando info...</span></div>
    {:else if detail}
      <div class="detail-hero">
        <img class="detail-cover" src={detail.image} alt={detail.title} onerror={(e) => onImgError(e, detail.image)} />
        <div class="detail-info">
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="lib-btn-wrap">
            {#if $animeLibrary.some(e => e.id === detail.id)}
              <button class="lib-btn in-lib" onclick={() => toggleAnimeLibrary(detail)}>
                <svg viewBox="0 0 24 24" fill="currentColor"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
                En biblioteca
              </button>
            {:else}
              <button class="lib-btn" onclick={() => toggleAnimeLibrary(detail)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/></svg>
                Añadir a biblioteca
              </button>
            {/if}
          </div>
          <h1 class="detail-title">{detail.title}</h1>
          <div class="detail-meta">
            <span class="meta-badge" style="background:{typeColor(detail.anime_type)}22; color:{typeColor(detail.anime_type)};">{detail.anime_type || 'TV'}</span>
            {#if detail.status}<span class="meta-badge status">{detail.status}</span>{/if}
          </div>
          {#if detail.genres?.length}
            <div class="detail-genres">{#each detail.genres as g}<span class="genre-tag">{g}</span>{/each}</div>
          {/if}
          {#if detail.synopsis}<p class="detail-synopsis">{detail.synopsis}</p>{/if}
        </div>
      </div>
      <div class="eps-section">
        <h2 class="eps-title">
          Episodios
          {#if detail.episodes?.length}<span class="eps-count">{detail.episodes.length}</span>{/if}
          {#if loading}<span class="eps-loading"><div class="spinner-sm"></div></span>{/if}
          <button class="watch-all-btn" class:all-watched={allWatched} onclick={markAllWatched} title="Marcar todos como vistos">
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 10 8 14 16 6"/></svg>
            {allWatched ? 'Todo visto' : 'Marcar todo visto'}
          </button>
        </h2>
        {#if !detail.episodes?.length && !loading}
          <p class="no-eps">No se encontraron episodios</p>
        {:else}
          <div class="eps-list">
            {#each detail.episodes as ep}
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="ep-row" class:is-watched={watchedSet.has(ep.id)} onclick={() => openEpisode(ep)}>
                <span class="ep-num">
                  Ep. {ep.number}
                  {#if watchedSet.has(ep.id)}<span class="watched-tag">Visto</span>{/if}
                </span>
                {#if ep.title}
                  <span class="ep-title-text">{ep.title}</span>
                {/if}
                {#if epDates[ep.number]}
                  <span class="ep-date">{epDates[ep.number]}</span>
                {:else if ep.airdate}
                  <span class="ep-date">{ep.airdate}</span>
                {/if}
                {#if ep.length_min}
                  <span class="ep-dur">{ep.length_min}min</span>
                {/if}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                <span class="ep-torrent-btn" title="Buscar torrent en Nyaa.si"
                  onclick={(e) => { e.stopPropagation(); torrentModal = { animeTitle: detail?.title ?? '', episodeNumber: ep.number }; }}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                    <polyline points="8 17 12 21 16 17"/><line x1="12" y1="3" x2="12" y2="21"/>
                    <polyline points="20 7 12 3 4 7"/>
                  </svg>
                  Torrent
                </span>
                {#if detailSource === 'kitsu'}
                  <svg class="ep-play kitsu-play" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                    <polyline points="8 17 12 21 16 17"/><line x1="12" y1="3" x2="12" y2="21"/>
                    <polyline points="20 7 12 3 4 7"/>
                  </svg>
                {:else}
                  <svg class="ep-play" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                {/if}
              </div>
            {/each}
          </div>
          {#if kitsuEpsMore && detailSource === 'kitsu'}
            <div class="load-more-wrap">
              <button class="load-more-btn" onclick={loadMoreKitsuEps}>Cargar más episodios</button>
            </div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
</div>

{:else if subView === 'player'}
<div class="view player-view">
  <div class="topbar">
    <button class="back-btn" onclick={goBack}>← Volver</button>
    <span class="topbar-title">{detail?.title ?? ''} — Ep. {episode?.number ?? ''}</span>
  </div>
  <div class="player-body">
    {#if loading}
      <div class="player-placeholder"><div class="spinner large"></div><span>Buscando servidores...</span></div>
    {:else if error || streams.length === 0}
      <div class="player-placeholder error">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
        <span>No se encontraron servidores disponibles</span>
      </div>
    {:else}
      {#if extracting}
        <div class="player-placeholder"><div class="spinner large"></div><span>Extrayendo fuente directa...</span></div>
      {:else if directUrl}
        <div class="iframe-wrap">
          <video src={directUrl} controls autoplay playsinline style="position:absolute;inset:0;width:100%;height:100%;background:#000;"></video>
        </div>
      {:else if selectedServer}
        {#if extractError}
          <div class="extract-warn">Stape: {extractError} — usando servidor alternativo</div>
        {/if}
        {#if streams.length > 1}
          <div class="server-bar">
            <span class="server-label">Servidor:</span>
            {#each streams as stream}
              <button class="server-btn" class:active={selectedServer?.url === stream.url} onclick={() => selectServer(stream)}>
                {stream.server}
              </button>
            {/each}
          </div>
        {/if}
        <div class="iframe-wrap">
          <iframe src={selectedServer.url} title="Reproductor" allowfullscreen frameborder="0" scrolling="no" allow="autoplay; fullscreen"></iframe>
        </div>
      {/if}
    {/if}
  </div>
</div>
{/if}

<!-- ── Modal de búsqueda de torrents ── -->
{#if torrentModal}
  <TorrentModal
    animeTitle={torrentModal.animeTitle}
    episodeNumber={torrentModal.episodeNumber}
    onClose={() => torrentModal = null}
    onSelect={handleTorrentSelect}
  />
{/if}

<!-- ── Reproductor torrent (overlay sobre el player actual) ── -->
{#if torrentPlayer}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="torrent-overlay">
    <div class="torrent-player-wrap">
      <div class="torrent-header">
        <div class="torrent-title">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <polyline points="8 17 12 21 16 17"/><line x1="12" y1="3" x2="12" y2="21"/>
            <polyline points="20 7 12 3 4 7"/>
          </svg>
          {torrentPlayer.fileName}
        </div>
        <button class="ext-btn" onclick={openInExternal} title="Abrir en mpv/VLC">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M15 3h6v6"/><path d="M10 14 21 3"/>
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
          </svg>
          Externo
        </button>
        <button class="torrent-close" onclick={closeTorrentPlayer}>✕</button>
      </div>

      {#if torrentPlayer.fileUrl}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="video-container" onmousemove={revealControls} onmouseleave={() => showSubMenu = false}>
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            class="torrent-video"
            src={torrentPlayer.fileUrl}
            autoplay playsinline
            onerror={onVideoError}
            ontimeupdate={onTimeUpdate}
            ondurationchange={onDurationChange}
            onprogress={onProgress}
            onplay={onVideoPlay}
            onpause={onVideoPause}
            oncanplay={onCanPlay}
            bind:this={videoEl}
          ></video>

          {#if seekLoading}
            <div class="seek-loading">
              <div class="spinner large"></div>
            </div>
          {/if}

          {#if currentSubLine}
            <div class="sub-overlay">{currentSubLine}</div>
          {:else if subLoadError && activeSub}
            <div class="sub-overlay sub-error">{subLoadError}</div>
          {/if}

          <!-- Controles personalizados -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="vid-controls" class:hidden={!showControls}>
            <!-- Barra de progreso -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="seek-track" onclick={seekTo}>
              <div class="seek-buf"  style="width:{videoDuration > 0 ? (bufferedEnd / videoDuration * 100) : 0}%"></div>
              <div class="seek-fill" style="width:{videoDuration > 0 ? (videoCurrent / videoDuration * 100) : 0}%">
                <div class="seek-thumb"></div>
              </div>
            </div>

            <div class="ctrl-row">
              <!-- Play / Pause -->
              <button class="ctrl-btn" onclick={togglePlay} title={videoPaused ? 'Reproducir' : 'Pausar'}>
                {#if videoPaused}
                  <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                {:else}
                  <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
                {/if}
              </button>

              <!-- Tiempo -->
              <span class="ctrl-time">{fmtTime(videoCurrent)} / {fmtTime(videoDuration)}</span>

              <div class="ctrl-spacer"></div>

              <!-- Subtítulos -->
              <div class="cc-wrap">
                {#if torrentPlayer._needsTransmux}
                  <button
                    class="cc-btn"
                    class:cc-active={!!torrentPlayer.activeSub}
                    onclick={(e) => { e.stopPropagation(); showSubMenu = !showSubMenu; }}
                    title="Subtítulos"
                  >
                    {#if probingSubs}
                      <span class="cc-spinner"></span>
                    {:else}
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="6" width="20" height="12" rx="2"/>
                        <path d="M7 12h4M15 12h2M7 16h2M13 16h4"/>
                      </svg>
                    {/if}
                    CC
                  </button>
                  {#if showSubMenu}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div class="cc-menu">
                      <button
                        class="cc-item"
                        class:cc-item-active={!torrentPlayer.activeSub}
                        onclick={() => { selectSub(null); showSubMenu = false; }}
                      >
                        <span class="cc-dot"></span>Off
                      </button>
                      {#each torrentPlayer.subs as s}
                        <button
                          class="cc-item"
                          class:cc-item-active={torrentPlayer.activeSub?.url === s.url}
                          onclick={() => { selectSub(s); showSubMenu = false; }}
                        >
                          <span class="cc-dot"></span>
                          {#if s.lang}<span class="cc-lang">{s.lang}</span>{/if}
                          {s.title || s.name || `Pista ${s.index + 1}`}
                        </button>
                      {/each}
                      <button
                        class="cc-item cc-rescan"
                        onclick={() => { showSubMenu = false; probeSubs(); }}
                        disabled={probingSubs}
                      >
                        <span class="cc-dot"></span>
                        {probingSubs ? 'Buscando…' : '↺ Re-detectar pistas'}
                      </button>
                    </div>
                  {/if}
                {/if}
              </div>
            </div>
          </div>
        </div>

        {#if torrentPlayer.videoError}
          <div class="video-err">{torrentPlayer.videoError}</div>
        {/if}
      {:else}
        <div class="torrent-waiting">
          <div class="spinner large"></div>
          {#if torrentPlayer.videoError}
            <span class="err-text">{torrentPlayer.videoError}</span>
          {:else}
            <span>Conectando al torrent…</span>
            <span class="hint">Buscando peers (puede tardar hasta 60s)</span>
          {/if}
        </div>
      {/if}

      {#if torrentPlayer.status}
        {@const s = torrentPlayer.status}
        <div class="torrent-progress-bar">
          <div class="torrent-prog-fill" style="width:{s.progress_pct}%"></div>
        </div>
        <div class="torrent-stats">
          <span class="t-state">{s.state}</span>
          <span>{formatBytes(s.downloaded)} / {formatBytes(s.total)}</span>
          <span class="t-speed">↓ {formatBytes(s.speed_bps)}/s</span>
          <span>👥 {s.peers} peers</span>
          <span class="t-pct">{s.progress_pct.toFixed(1)}%</span>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
    overflow: hidden;
  }

  /* ── Header (browse) ── */
  .header {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px 10px;
    background: var(--bg-low);
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .source-tabs {
    display: flex;
    gap: 6px;
  }

  .stab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: 8px;
    border: 1px solid var(--outline-dim);
    background: none;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    position: relative;
  }
  .stab svg { width: 13px; height: 13px; flex-shrink: 0; }
  .stab.active {
    background: var(--bg-card-high);
    color: var(--primary);
    border-color: var(--primary);
  }
  .stab.disabled { opacity: 0.4; cursor: default; }

  .torrent-badge {
    font-size: 8px;
    font-weight: 800;
    background: rgba(99,102,241,0.15);
    color: #818cf8;
    padding: 1px 5px;
    border-radius: 4px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  /* ── Buscador ── */
  .search-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-card);
    border: 1px solid var(--outline-dim);
    border-radius: 8px;
    padding: 0 10px;
    transition: border-color 0.15s;
  }
  .search-wrap:focus-within { border-color: var(--primary); }

  .search-ico { width: 14px; height: 14px; color: var(--text-muted); flex-shrink: 0; }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text);
    font-size: 13px;
    padding: 8px 0;
    outline: none;
  }
  .search-input::placeholder { color: var(--text-muted); }

  .search-clear {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 11px;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .search-clear:hover { color: var(--text); background: var(--bg-card-high); }

  /* ── Pestañas browse ── */
  .browse-tabs {
    display: flex;
    gap: 2px;
    padding: 0 16px;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }

  .btab {
    padding: 7px 14px;
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
  .btab:hover { color: var(--text); }
  .btab.active { color: var(--primary); border-bottom-color: var(--primary); }

  /* ── Etiqueta sección ── */
  .section-label {
    padding: 8px 16px 4px;
    font-size: 11px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .section-label strong { color: var(--text); font-weight: 700; }

  /* ── Cargar más ── */
  .load-more-wrap {
    display: flex;
    justify-content: center;
    padding: 16px 0 8px;
  }

  .load-more-btn {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 24px;
    font-size: 12px;
    font-weight: 700;
    border-radius: 8px;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .load-more-btn:hover:not(:disabled) { background: var(--bg-card-high); color: var(--text); }
  .load-more-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ── Scroll area ── */
  .scroll-area {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px 24px;
  }

  /* ── Cards grid ── */
  .cards-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 4px 0;
  }

  .acard {
    display: flex;
    flex-direction: column;
    gap: 6px;
    cursor: pointer;
    width: 148px;
  }
  .acard:hover .acard-cover { opacity: 0.85; transform: scale(1.02); }

  .acard-cover-wrap {
    position: relative;
    width: 148px;
    height: 210px;
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-card);
  }

  .acard-cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: opacity 0.15s, transform 0.15s;
  }

  .acard-type {
    position: absolute;
    top: 6px;
    right: 6px;
    font-size: 9px;
    font-weight: 800;
    padding: 2px 6px;
    border-radius: 4px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .acard-ep-badge {
    position: absolute;
    bottom: 0; left: 0; right: 0;
    background: linear-gradient(to top, rgba(0,0,0,0.9) 0%, transparent 100%);
    padding: 18px 7px 6px;
    font-size: 10px;
    font-weight: 700;
    color: #e879f9;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .acard-title {
    font-size: 11px;
    color: var(--text);
    font-weight: 500;
    line-height: 1.3;
    max-width: 148px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Topbar (detail/player) ── */
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

  .topbar-title {
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Detail ── */
  .detail-hero {
    display: flex;
    gap: 18px;
    padding: 16px 0 20px;
  }

  .detail-cover {
    width: 140px;
    height: 200px;
    object-fit: cover;
    border-radius: 8px;
    flex-shrink: 0;
    background: var(--bg-card);
  }

  .detail-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .detail-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--text);
    line-height: 1.3;
    margin: 0;
  }

  .detail-meta {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .meta-badge {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 5px;
    letter-spacing: 0.03em;
  }
  .meta-badge.status {
    background: rgba(255,255,255,0.06);
    color: var(--text-muted);
  }

  .detail-genres {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .genre-tag {
    font-size: 10px;
    background: rgba(255,255,255,0.06);
    color: var(--text-muted);
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--outline-dim);
  }

  .detail-synopsis {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.6;
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* ── Episodes ── */
  .eps-section {
    border-top: 1px solid var(--outline-dim);
    padding-top: 12px;
  }

  .eps-count {
    font-size: 10px;
    background: var(--bg-card);
    color: var(--text-muted);
    padding: 1px 7px;
    border-radius: 99px;
    font-weight: 600;
  }

  .eps-loading { display: flex; align-items: center; }

  .no-eps {
    color: var(--text-muted);
    font-size: 12px;
    padding: 12px 0;
  }

  .eps-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ep-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 7px;
    cursor: pointer;
    transition: background 0.12s;
    border: 1px solid transparent;
  }
  .ep-row:hover {
    background: var(--bg-card);
    border-color: var(--outline-dim);
  }

  .eps-title {
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
    margin: 0 0 10px;
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }

  .watch-all-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 10px;
    border-radius: 5px;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    white-space: nowrap;
  }
  .watch-all-btn svg { width: 11px; height: 11px; }
  .watch-all-btn:hover { color: var(--text); background: var(--bg-card-high); }
  .watch-all-btn.all-watched { color: #7c3aed; opacity: 0.6; cursor: default; }

  .ep-row.is-watched .ep-num { color: var(--text-muted); }

  .watched-tag {
    font-size: 9px;
    font-weight: 700;
    background: rgba(99,102,241,0.15);
    color: #818cf8;
    padding: 1px 5px;
    border-radius: 3px;
    margin-left: 5px;
    letter-spacing: 0.03em;
  }

  .ep-num {
    font-size: 13px;
    color: var(--text);
    font-weight: 500;
    flex: 1;
    display: flex;
    align-items: center;
  }

  .ep-date {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    margin-right: 6px;
  }

  .ep-play {
    width: 14px;
    height: 14px;
    color: var(--primary);
    opacity: 0;
    transition: opacity 0.12s;
    flex-shrink: 0;
  }
  .ep-row:hover .ep-play { opacity: 1; }

  /* ── Player ── */
  .player-view { background: #000; }

  .player-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .server-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    background: var(--bg-low);
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .server-label {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 600;
    margin-right: 4px;
  }

  .server-btn {
    padding: 4px 12px;
    border-radius: 5px;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .server-btn:hover { color: var(--text); background: var(--bg-card-high); }
  .server-btn.active {
    background: var(--primary);
    color: #fff;
    border-color: var(--primary);
  }

  .iframe-wrap {
    flex: 1;
    position: relative;
  }

  /* ── Botón biblioteca ── */
  .lib-btn-wrap { margin-bottom: 8px; }

  .lib-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 700;
    padding: 5px 12px;
    border-radius: 6px;
    cursor: pointer;
    border: 1px solid var(--outline-dim);
    background: var(--bg-card);
    color: var(--text-muted);
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .lib-btn svg { width: 13px; height: 13px; flex-shrink: 0; }
  .lib-btn:hover { color: var(--text); background: var(--bg-card-high); }
  .lib-btn.in-lib {
    background: rgba(99,102,241,0.15);
    color: #818cf8;
    border-color: rgba(99,102,241,0.4);
  }
  .lib-btn.in-lib:hover {
    background: rgba(239,68,68,0.12);
    color: #f87171;
    border-color: rgba(239,68,68,0.4);
  }

  .iframe-wrap iframe {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    border: none;
  }

  /* ── Estados ── */
  .player-placeholder {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .player-placeholder svg { width: 32px; height: 32px; }
  .player-placeholder.error { color: #f87171; }

  .status-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-muted);
    font-size: 13px;
    padding: 60px 20px;
  }
  .status-center.error { color: #f87171; }
  .status-center svg { width: 28px; height: 28px; }

  .retry-btn {
    margin-top: 4px;
    padding: 6px 16px;
    border-radius: 7px;
    border: 1px solid rgba(248,113,113,0.4);
    background: rgba(248,113,113,0.08);
    color: #f87171;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .retry-btn:hover { background: rgba(248,113,113,0.15); }

  .spinner {
    width: 24px; height: 24px;
    border: 3px solid var(--outline-dim);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  .spinner.large { width: 40px; height: 40px; border-width: 4px; }

  .spinner-sm {
    width: 14px; height: 14px;
    border: 2px solid var(--outline-dim);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    display: inline-block;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Título episodio Kitsu ── */
  .ep-title-text {
    flex: 1; font-size: 11px; color: var(--text-muted);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    min-width: 0;
  }
  .ep-dur {
    font-size: 10px; color: var(--text-muted); flex-shrink: 0; opacity: 0.6;
  }
  .ep-play.kitsu-play {
    color: var(--primary, #f59e0b); opacity: 0.7;
  }

  /* ── Botón torrent en fila de episodio ── */
  .ep-torrent-btn {
    display: flex; align-items: center; gap: 4px;
    padding: 2px 6px; height: 22px; flex-shrink: 0;
    border-radius: 4px; cursor: pointer;
    font-size: 10px; font-weight: 600;
    color: var(--text-muted); opacity: 1;
    transition: color 0.12s, background 0.12s;
  }
  .ep-torrent-btn svg { width: 12px; height: 12px; flex-shrink: 0; }
  .ep-torrent-btn:hover { color: var(--primary); background: rgba(245,158,11,0.12); }

  /* ── Reproductor torrent overlay ── */
  .torrent-overlay {
    position: fixed; inset: 0; z-index: 8000;
    background: rgba(0,0,0,0.85);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(6px);
  }

  .torrent-player-wrap {
    width: min(960px, 96vw);
    max-height: 90vh;
    background: var(--bg, #0f0f0f);
    border: 1px solid var(--outline-dim);
    border-radius: 12px;
    display: flex; flex-direction: column;
    overflow: hidden;
  }

  .torrent-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--outline-dim);
    flex-shrink: 0;
  }
  .torrent-title {
    display: flex; align-items: center; gap: 8px;
    font-size: 12px; font-weight: 700; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .torrent-title svg { width: 15px; height: 15px; color: var(--primary); flex-shrink: 0; }
  .torrent-close {
    background: none; border: none; color: var(--text-muted);
    font-size: 14px; cursor: pointer; padding: 4px 8px; border-radius: 4px; flex-shrink: 0;
  }
  .torrent-close:hover { color: var(--text); }

  .video-container {
    position: relative;
    width: 100%;
    background: #000;
    flex-shrink: 0;
    line-height: 0;
  }

  .torrent-video {
    width: 100%;
    max-height: 60vh;
    display: block;
  }

  .sub-overlay {
    position: absolute;
    bottom: 64px;
    left: 5%;
    right: 5%;
    text-align: center;
    pointer-events: none;
    color: #fff;
    font-size: 15px;
    font-weight: 500;
    line-height: 1.45;
    white-space: pre-wrap;
    text-shadow: 1px 1px 3px #000, -1px -1px 3px #000, 0 0 6px #000;
    background: rgba(0,0,0,0.35);
    padding: 3px 10px;
    border-radius: 4px;
  }
  .sub-overlay.sub-error {
    font-size: 11px;
    color: #fca5a5;
    background: rgba(127,0,0,0.5);
  }

  .seek-loading {
    position: absolute; inset: 0;
    display: flex; align-items: center; justify-content: center;
    background: rgba(0,0,0,0.5);
    z-index: 5;
  }

  /* ── Controles personalizados ── */
  .vid-controls {
    position: absolute; bottom: 0; left: 0; right: 0;
    background: linear-gradient(transparent, rgba(0,0,0,0.75));
    padding: 20px 10px 8px;
    display: flex; flex-direction: column; gap: 5px;
    transition: opacity 0.25s;
    z-index: 10;
  }
  .vid-controls.hidden { opacity: 0; pointer-events: none; }

  .seek-track {
    position: relative; height: 4px; border-radius: 2px;
    background: rgba(255,255,255,0.2); cursor: pointer;
    flex-shrink: 0;
    transition: height 0.15s;
  }
  .seek-track:hover { height: 6px; }
  .seek-buf {
    position: absolute; top: 0; left: 0; height: 100%; border-radius: 2px;
    background: rgba(255,255,255,0.3); pointer-events: none;
  }
  .seek-fill {
    position: absolute; top: 0; left: 0; height: 100%; border-radius: 2px;
    background: var(--primary, #f59e0b); pointer-events: none;
    display: flex; align-items: center; justify-content: flex-end;
  }
  .seek-thumb {
    width: 12px; height: 12px; border-radius: 50%;
    background: var(--primary, #f59e0b);
    box-shadow: 0 0 4px rgba(0,0,0,0.5);
    flex-shrink: 0; margin-right: -6px;
    opacity: 0; transition: opacity 0.15s;
  }
  .seek-track:hover .seek-thumb { opacity: 1; }

  .ctrl-row {
    display: flex; align-items: center; gap: 8px;
    height: 32px;
  }
  .ctrl-btn {
    background: none; border: none; color: #fff;
    cursor: pointer; padding: 4px; border-radius: 4px;
    display: flex; align-items: center; justify-content: center;
    flex-shrink: 0;
    transition: background 0.1s;
  }
  .ctrl-btn:hover { background: rgba(255,255,255,0.15); }
  .ctrl-btn svg { width: 18px; height: 18px; }

  .ctrl-time {
    font-size: 11px; color: rgba(255,255,255,0.85);
    font-variant-numeric: tabular-nums; white-space: nowrap;
  }
  .ctrl-spacer { flex: 1; }

  .torrent-waiting {
    height: 300px; display: flex; flex-direction: column;
    align-items: center; justify-content: center;
    gap: 12px; color: var(--text); font-size: 14px;
  }
  .torrent-waiting .hint { color: var(--text-muted); font-size: 11px; }
  .torrent-waiting .err-text { color: #f87171; font-size: 12px; max-width: 400px; text-align: center; }

  .ext-btn {
    display: flex; align-items: center; gap: 6px;
    background: var(--primary); color: #000;
    border: none; border-radius: 6px;
    font-size: 11px; font-weight: 700;
    padding: 6px 12px; cursor: pointer;
    margin-right: 8px;
  }
  .ext-btn svg { width: 12px; height: 12px; }

  .video-err {
    background: rgba(248,113,113,0.12); color: #f87171;
    font-size: 11px; padding: 8px 16px; border-top: 1px solid rgba(248,113,113,0.3);
    flex-shrink: 0;
  }

  /* ── CC button (in-player subtitle selector) ── */
  .cc-wrap {
    position: relative;
    flex-shrink: 0;
  }

  .cc-btn {
    display: flex; align-items: center; gap: 4px;
    background: rgba(0,0,0,0.65);
    color: rgba(255,255,255,0.85);
    border: 1px solid rgba(255,255,255,0.25);
    border-radius: 5px;
    padding: 4px 9px;
    font-size: 11px; font-weight: 700;
    cursor: pointer;
    letter-spacing: 0.04em;
    backdrop-filter: blur(4px);
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .cc-btn svg { width: 14px; height: 14px; }
  .cc-btn:hover { background: rgba(0,0,0,0.85); color: #fff; border-color: rgba(255,255,255,0.5); }
  .cc-btn.cc-active { background: var(--primary, #f59e0b); color: #000; border-color: var(--primary, #f59e0b); }
  .cc-btn.cc-detect { opacity: 0.7; }
  .cc-btn.cc-detect:hover { opacity: 1; }
  .cc-btn:disabled { cursor: wait; }

  .cc-spinner {
    width: 12px; height: 12px;
    border: 2px solid rgba(255,255,255,0.3);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  .cc-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    right: 0;
    background: rgba(12,12,12,0.96);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 8px;
    overflow: hidden;
    min-width: 180px;
    box-shadow: 0 6px 24px rgba(0,0,0,0.7);
    backdrop-filter: blur(8px);
  }

  .cc-item {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 9px 14px;
    background: none; border: none;
    border-bottom: 1px solid rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.75);
    font-size: 12px; text-align: left; cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .cc-item:last-child { border-bottom: none; }
  .cc-item:hover { background: rgba(255,255,255,0.07); color: #fff; }
  .cc-item.cc-item-active { color: var(--primary, #f59e0b); font-weight: 700; }
  .cc-item.cc-item-active .cc-dot { background: var(--primary, #f59e0b); }
  .cc-item.cc-rescan { color: var(--text-muted); font-size: 10px; border-top: 1px solid var(--outline-dim); margin-top: 2px; }
  .cc-item.cc-rescan:hover { color: var(--text); }

  .cc-dot {
    width: 6px; height: 6px; border-radius: 50%;
    background: rgba(255,255,255,0.2); flex-shrink: 0;
  }

  .cc-lang {
    font-size: 9px; font-weight: 800;
    background: rgba(245,158,11,0.15); color: var(--primary, #f59e0b);
    padding: 1px 5px; border-radius: 3px; letter-spacing: 0.05em;
    text-transform: uppercase; flex-shrink: 0;
  }

  .torrent-progress-bar {
    height: 3px; background: var(--outline-dim); flex-shrink: 0;
  }
  .torrent-prog-fill {
    height: 100%; background: var(--primary); transition: width 1s linear;
  }

  .torrent-stats {
    display: flex; align-items: center; gap: 16px; flex-wrap: wrap;
    padding: 8px 16px;
    font-size: 11px; color: var(--text-muted);
    flex-shrink: 0;
  }
  .t-state { text-transform: capitalize; color: var(--text); font-weight: 600; }
  .t-speed { color: #4ade80; }
  .t-pct   { color: var(--primary); font-weight: 700; margin-left: auto; }
</style>
