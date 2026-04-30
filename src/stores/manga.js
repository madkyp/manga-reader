import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';

// ── Auto-descarga ─────────────────────────────────────────────────────────────
// { [mangaId]: true } — mangas con auto-descarga activada
function loadAutoDownloadConfig() {
    try { return JSON.parse(localStorage.getItem('foundry_auto_dl') || '{}'); } catch { return {}; }
}
function saveAutoDownloadConfig(c) {
    try { localStorage.setItem('foundry_auto_dl', JSON.stringify(c)); } catch {}
}
export const autoDownloadConfig = writable(loadAutoDownloadConfig());

export function toggleAutoDownload(mangaId) {
    autoDownloadConfig.update(cfg => {
        const next = { ...cfg, [mangaId]: !cfg[mangaId] };
        saveAutoDownloadConfig(next);
        return next;
    });
}

// ── AniList — mapeo local ID → AniList media_id ───────────────────────────────
// { [mangaId]: anilistId }
function loadAniListMap() {
    try { return JSON.parse(localStorage.getItem('foundry_anilist_map') || '{}'); } catch { return {}; }
}
function saveAniListMap(m) {
    try { localStorage.setItem('foundry_anilist_map', JSON.stringify(m)); } catch {}
}
export const anilistMap = writable(loadAniListMap());

export function setAniListId(mangaId, anilistId) {
    anilistMap.update(m => {
        const next = { ...m, [mangaId]: anilistId };
        saveAniListMap(next);
        return next;
    });
}

// ── Ruta de descarga ──────────────────────────────────────────────────────────
function loadDownloadPath() {
    try { return localStorage.getItem('foundry_dl_path') || ''; } catch { return ''; }
}
export const downloadPath = writable(loadDownloadPath());
downloadPath.subscribe(p => {
    try { localStorage.setItem('foundry_dl_path', p); } catch {}
});

// ── Vista activa ──────────────────────────────────────────────────────────────
export const currentView = writable('home');
export const currentTab  = writable('latest');
export const previousView = writable('browse');

// ── Fuente activa ─────────────────────────────────────────────────────────────
// 'olympus' | 'cerberus' | 'taurus' | 'leercapitulo'
export const currentSource = writable('olympus');

// Comandos invoke por fuente
const SOURCE_CMDS = {
    olympus:      { latest: 'get_latest',          browse: 'get_browse',          info: 'get_info',          pages: 'get_pages' },
    cerberus:     { latest: 'cerberus_latest',      browse: 'cerberus_browse',     info: 'cerberus_info',     pages: 'cerberus_pages' },
    taurus:       { latest: 'taurus_latest',        browse: 'taurus_browse',       info: 'taurus_info',       pages: 'taurus_pages' },
    leercapitulo: { latest: 'leercapitulo_latest',  browse: 'leercapitulo_browse', info: 'leercapitulo_info', pages: 'leercapitulo_pages' },
};

function cmds() { return SOURCE_CMDS[get(currentSource)] ?? SOURCE_CMDS.olympus; }

// ── Manga seleccionado ────────────────────────────────────────────────────────
export const selectedManga  = writable(null);
export const currentManga   = writable(null);
export const currentChapter = writable(null);
export const readerPages    = writable([]);

// ── Caché de títulos ──────────────────────────────────────────────────────────
export const titleCache = writable({});

// ── Biblioteca ────────────────────────────────────────────────────────────────
// Cada entrada: { id, title, image, source }
function loadLibrary() {
    try { return JSON.parse(localStorage.getItem('foundry_library') || '[]'); } catch { return []; }
}
function saveLibrary(l) {
    try { localStorage.setItem('foundry_library', JSON.stringify(l)); } catch {}
}
export const library = writable(loadLibrary());

export function toggleLibrary(manga) {
    let adding = false;
    library.update(lib => {
        const exists = lib.some(e => e.id === manga.id);
        adding = !exists;
        const next = exists
            ? lib.filter(e => e.id !== manga.id)
            : [...lib, { id: manga.id, title: manga.title, image: manga.image }];
        saveLibrary(next);
        return next;
    });
    // Al añadir: inicializa estado de notif con capítulo actual para no generar falsa alerta
    if (!adding) return;
    const currentChaps = manga.chapters;
    if (currentChaps && currentChaps.length > 0) {
        notifState.update(s => {
            if (s[manga.id]) return s;
            const next = { ...s, [manga.id]: { lastChapterId: currentChaps[0].id, lastChapterTitle: currentChaps[0].title, unread: false, checkedAt: new Date().toISOString() } };
            saveNotifState(next);
            return next;
        });
    }
}

// ── Notificaciones ────────────────────────────────────────────────────────────
// { [mangaId]: { lastChapterId, lastChapterTitle, unread, checkedAt } }
function loadNotifState() {
    try { return JSON.parse(localStorage.getItem('foundry_notif') || '{}'); } catch { return {}; }
}
function saveNotifState(s) {
    try { localStorage.setItem('foundry_notif', JSON.stringify(s)); } catch {}
}
export const notifState     = writable(loadNotifState());
export const checkingNotifs = writable(false);
export const unreadCount    = derived(notifState, $s => Object.values($s).filter(v => v.unread).length);

const NOTIF_COOLDOWN_MS = 60 * 60 * 1000; // 1 hora

async function tryDesktopNotification(title, body) {
    try {
        let granted = await isPermissionGranted();
        if (!granted) {
            const perm = await requestPermission();
            granted = perm === 'granted';
        }
        if (granted) sendNotification({ title, body });
    } catch (e) {
        console.warn('Desktop notification failed:', e);
    }
}

export async function checkNotifications(force = false) {
    const lib = get(library);
    if (lib.length === 0) return;
    if (get(checkingNotifs)) return;
    checkingNotifs.set(true);

    const state = loadNotifState();
    let changed = false;

    for (const item of lib) {
        // Cooldown: saltar si se revisó hace menos de 1h (salvo force)
        const prev = state[item.id];
        if (!force && prev?.checkedAt) {
            const age = Date.now() - new Date(prev.checkedAt).getTime();
            if (age < NOTIF_COOLDOWN_MS) continue;
        }
        try {
            const source = item.id?.startsWith('cerberus-') ? 'cerberus'
                         : item.id?.startsWith('taurus-')   ? 'taurus'
                         : item.id?.startsWith('leer-')      ? 'leercapitulo'
                         : 'olympus';
            const data = await invoke(SOURCE_CMDS[source].info, { id: item.id });
            if (data.chapters && data.chapters.length > 0) {
                const first = data.chapters[0];
                if (!prev) {
                    state[item.id] = { lastChapterId: first.id, lastChapterTitle: first.title, unread: false, checkedAt: new Date().toISOString() };
                } else if (prev.lastChapterId !== first.id) {
                    state[item.id] = { ...prev, lastChapterId: first.id, lastChapterTitle: first.title, unread: true, checkedAt: new Date().toISOString() };
                    tryDesktopNotification(
                        item.title,
                        `Nuevo capítulo disponible: ${first.title || 'Cap. nuevo'}`
                    );
                    // Auto-descarga si está activada para este manga
                    const autoDl = loadAutoDownloadConfig();
                    if (autoDl[item.id]) {
                        invoke('download_chapter', {
                            mangaId:      item.id,
                            mangaTitle:   item.title || '',
                            chapterId:    first.id,
                            chapterTitle: first.title || '',
                            downloadPath: null,
                        }).catch(e => console.warn('[auto-dl]', item.title, e));
                    }
                } else {
                    state[item.id] = { ...prev, checkedAt: new Date().toISOString() };
                }
                changed = true;
            }
        } catch (e) {
            console.error('Error checking notif for', item.id, e);
        }
    }

    if (changed) { saveNotifState(state); notifState.set(state); }
    checkingNotifs.set(false);
}

export function markNotifRead(mangaId) {
    notifState.update(s => {
        if (!s[mangaId]) return s;
        const next = { ...s, [mangaId]: { ...s[mangaId], unread: false } };
        saveNotifState(next);
        return next;
    });
}

export function markAllNotifsRead() {
    notifState.update(s => {
        const next = Object.fromEntries(Object.entries(s).map(([k, v]) => [k, { ...v, unread: false }]));
        saveNotifState(next);
        return next;
    });
}

// ── Historial de lectura ──────────────────────────────────────────────────────
// Cada entrada: { manga: MangaItem, chapter: { id, title, date }, timestamp }
function loadHistory() {
    try { return JSON.parse(localStorage.getItem('readHistory') || '[]'); } catch { return []; }
}
function saveHistory(h) {
    try { localStorage.setItem('readHistory', JSON.stringify(h)); } catch {}
}
export const readHistory = writable(loadHistory());

function loadStats() {
    try { return JSON.parse(localStorage.getItem('readStats') || '{"chaptersTotal":0,"activeDays":{}}'); } catch { return { chaptersTotal: 0, activeDays: {} }; }
}
function saveStats(s) {
    try { localStorage.setItem('readStats', JSON.stringify(s)); } catch {}
}
export const readStats = writable(loadStats());

export function addToHistory(manga, chapter) {
    // Actualiza historial
    readHistory.update(h => {
        const filtered = h.filter(e => e.manga.id !== manga.id);
        const entry = { manga: { id: manga.id, title: manga.title, image: manga.image }, chapter: { id: chapter.id, title: chapter.title }, timestamp: Date.now() };
        const next = [entry, ...filtered].slice(0, 30);
        saveHistory(next);
        return next;
    });
    // Actualiza estadísticas
    readStats.update(s => {
        const today = new Date().toISOString().slice(0, 10);
        const next = { chaptersTotal: (s.chaptersTotal || 0) + 1, activeDays: { ...s.activeDays, [today]: true } };
        saveStats(next);
        return next;
    });
}

// ── Listas paginadas ──────────────────────────────────────────────────────────
export const latestItems   = writable([]);
export const browseItems   = writable([]);
export const latestPage    = writable(1);
export const browsePage    = writable(1);
export const latestHasMore = writable(false);
export const browseHasMore = writable(false);

export const loadingLatest = writable(false);
export const loadingBrowse = writable(false);
export const loadingDetail = writable(false);
export const loadingReader = writable(false);
export const browseError   = writable(null);

// ── Caché de datos por fuente ─────────────────────────────────────────────────
const sourceDataCache = {};

// ── Acciones ──────────────────────────────────────────────────────────────────

// ── Timestamps "primera vez visto en latest" ──────────────────────────────────
// Cache: { [id]: { d1: ISO, d2: ISO } }
// d1 = último capítulo detectado, d2 = capítulo anterior.
// Si el item lleva > 8h en caché y reaparece en página 1 → nuevo capítulo detectado
// → shift: d1 pasa a d2, d1 = ahora.
const STALE_MS = 8 * 60 * 60 * 1000; // 8 horas

function loadTsCache() {
    try { return JSON.parse(localStorage.getItem('foundry_latest_ts') || '{}'); } catch { return {}; }
}
function saveTsCache(c) {
    try { localStorage.setItem('foundry_latest_ts', JSON.stringify(c)); } catch {}
}

function stampItems(items, page) {
    const cache = loadTsCache();
    const now   = new Date().toISOString();
    let changed = false;

    const stamped = items.map(item => {
        if (item.date) return item;  // el scraper ya trajo fecha real

        const entry = cache[item.id];
        // Migración: entradas antiguas eran strings ISO en vez de objetos
        const d1 = typeof entry === 'string' ? entry : entry?.d1 ?? null;
        const d2 = typeof entry === 'string' ? ''    : entry?.d2 ?? '';

        if (!d1) {
            // Primera vez visto
            cache[item.id] = { d1: now, d2: '' };
            changed = true;
            return { ...item, date: now, date2: '' };
        }

        if (page === 1) {
            const ageMs = Date.now() - new Date(d1).getTime();
            if (ageMs > STALE_MS) {
                // Nuevo capítulo detectado: d1 anterior pasa a slot derecho
                cache[item.id] = { d1: now, d2: d1 };
                changed = true;
                return { ...item, date: now, date2: d1 };
            }
        }

        // Sin cambio: devuelve timestamps ya conocidos
        return { ...item, date: d1, date2: item.date2 || d2 };
    });

    if (changed) saveTsCache(cache);
    return stamped;
}

export async function loadLatest(reset = false) {
    if (get(loadingLatest)) return;
    if (reset) { latestItems.set([]); latestPage.set(1); browseError.set(null); }

    loadingLatest.set(true);
    const page = get(latestPage);

    try {
        const data = await invoke(cmds().latest, { page });
        const stamped = stampItems(data.results, page);
        titleCache.update(cache => {
            stamped.forEach(item => { if (item.id && item.title) cache[item.id] = item.title; });
            return cache;
        });
        latestItems.update(items => reset ? stamped : [...items, ...stamped]);
        latestHasMore.set(data.hasMore);
        if (data.hasMore) latestPage.update(p => p + 1);
    } catch (e) {
        console.error('Error cargando latest:', e);
        browseError.set(String(e));
        latestHasMore.set(false);
    } finally {
        loadingLatest.set(false);
    }
}

export async function loadBrowse(reset = false) {
    if (get(loadingBrowse)) return;
    if (reset) { browseItems.set([]); browsePage.set(1); }

    loadingBrowse.set(true);
    const page = get(browsePage);

    try {
        const data = await invoke(cmds().browse, { page });
        titleCache.update(cache => {
            data.results.forEach(item => { if (item.id && item.title) cache[item.id] = item.title; });
            return cache;
        });
        browseItems.update(items => reset ? data.results : [...items, ...data.results]);
        browseHasMore.set(data.hasMore);
        if (data.hasMore) browsePage.update(p => p + 1);
    } catch (e) {
        console.error('Error cargando browse (página', page, '):', e);
        // Avanza la página para no quedarse atascado en páginas que fallan individualmente
        browsePage.update(p => p + 1);
    } finally {
        loadingBrowse.set(false);
    }
}

// Cambia la fuente activa — guarda el estado actual y restaura el de la nueva fuente
export function switchSource(source) {
    const prev = get(currentSource);
    if (prev === source) return;

    // Guarda el estado de la fuente actual
    sourceDataCache[prev] = {
        latestItems:   get(latestItems),
        browseItems:   get(browseItems),
        latestPage:    get(latestPage),
        browsePage:    get(browsePage),
        latestHasMore: get(latestHasMore),
        browseHasMore: get(browseHasMore),
    };

    currentSource.set(source);
    currentTab.set('latest');

    // Restaura el estado de la fuente destino si existe
    const cached = sourceDataCache[source];
    if (cached) {
        latestItems.set(cached.latestItems);
        browseItems.set(cached.browseItems);
        latestPage.set(cached.latestPage);
        browsePage.set(cached.browsePage);
        latestHasMore.set(cached.latestHasMore);
        browseHasMore.set(cached.browseHasMore);
    } else {
        latestItems.set([]);
        browseItems.set([]);
        latestPage.set(1);
        browsePage.set(1);
        latestHasMore.set(false);
        browseHasMore.set(false);
    }
}

export async function openManga(item) {
    previousView.set(get(currentView));
    selectedManga.set(item);
    currentManga.set(null);
    currentView.set('detail');
    loadingDetail.set(true);

    // Detecta la fuente por el prefijo del ID
    const source = item.id?.startsWith('cerberus-') ? 'cerberus'
                 : item.id?.startsWith('taurus-')   ? 'taurus'
                 : item.id?.startsWith('leer-')      ? 'leercapitulo'
                 : 'olympus';
    const infoCmd = SOURCE_CMDS[source].info;

    try {
        const data = await invoke(infoCmd, { id: item.id });
        if (!data.title || data.title === item.id) {
            const cache = get(titleCache);
            data.title = cache[item.id] || item.title || item.id;
        }
        currentManga.set(data);
    } catch (e) {
        console.error('Error cargando detalle:', e);
        currentManga.set({ ...item, chapters: [], genres: [], description: '' });
    } finally {
        loadingDetail.set(false);
    }
}

export async function openChapter(chapter, mangaId) {
    // Guarda en historial usando el manga actualmente cargado
    const manga = get(selectedManga) || get(currentManga);
    if (manga) addToHistory(manga, chapter);

    currentChapter.set(chapter);
    readerPages.set([]);
    currentView.set('reader');
    loadingReader.set(true);

    // Detecta la fuente por el prefijo del mangaId
    const isCerberus = mangaId?.startsWith('cerberus-');
    const isTaurus   = mangaId?.startsWith('taurus-');
    const isLeer     = mangaId?.startsWith('leer-');
    const source     = isCerberus ? 'cerberus' : isTaurus ? 'taurus' : isLeer ? 'leercapitulo' : 'olympus';
    const pagesCmd   = SOURCE_CMDS[source].pages;

    try {
        let pages;
        if (source === 'cerberus' || source === 'taurus' || source === 'leercapitulo') {
            pages = await invoke(pagesCmd, { chapterId: chapter.id });
        } else {
            pages = await invoke(pagesCmd, { chapterId: chapter.id, mangaId });
        }
        readerPages.set(pages);
    } catch (e) {
        console.error('Error cargando capítulo:', e);
    } finally {
        loadingReader.set(false);
    }
}

export function goBack() {
    currentView.update(v => {
        if (v === 'reader') return 'detail';
        if (v === 'detail') return get(previousView) || 'browse';
        return 'browse';
    });
}
