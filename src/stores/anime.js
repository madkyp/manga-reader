import { writable, derived, get } from 'svelte/store';
import { currentView } from './manga.js';

async function inv(cmd, args) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke(cmd, args);
}

// ── Biblioteca anime ──────────────────────────────────────────────────────────
function loadAnimeLibrary() {
    try { return JSON.parse(localStorage.getItem('foundry_anime_library') || '[]'); } catch { return []; }
}
function saveAnimeLibrary(l) {
    try { localStorage.setItem('foundry_anime_library', JSON.stringify(l)); } catch {}
}
export const animeLibrary = writable(loadAnimeLibrary());

export function toggleAnimeLibrary(item) {
    animeLibrary.update(lib => {
        const exists = lib.some(e => e.id === item.id);
        const next = exists
            ? lib.filter(e => e.id !== item.id)
            : [...lib, { id: item.id, title: item.title, image: item.image }];
        saveAnimeLibrary(next);
        return next;
    });
}

// ── Notificaciones anime ──────────────────────────────────────────────────────
// { [animeId]: { lastEpisode, unread, checkedAt } }
function loadAnimeNotif() {
    try { return JSON.parse(localStorage.getItem('foundry_anime_notif') || '{}'); } catch { return {}; }
}
function saveAnimeNotif(s) {
    try { localStorage.setItem('foundry_anime_notif', JSON.stringify(s)); } catch {}
}
export const animeNotifState     = writable(loadAnimeNotif());
export const checkingAnimeNotifs = writable(false);
export const animeUnreadCount    = derived(animeNotifState, $s => Object.values($s).filter(v => v.unread).length);

const NOTIF_COOLDOWN_MS = 60 * 60 * 1000;

export async function checkAnimeNotifications(force = false) {
    const lib = get(animeLibrary);
    if (lib.length === 0) return;
    if (get(checkingAnimeNotifs)) return;
    checkingAnimeNotifs.set(true);

    const state = loadAnimeNotif();
    let changed = false;

    for (const item of lib) {
        const prev = state[item.id];
        if (!force && prev?.checkedAt) {
            const age = Date.now() - new Date(prev.checkedAt).getTime();
            if (age < NOTIF_COOLDOWN_MS) continue;
        }
        try {
            const json = await inv('animeflv_detail', { slug: item.id });
            const detail = JSON.parse(json);
            if (detail.episodes && detail.episodes.length > 0) {
                const latest = detail.episodes[0].number;
                if (!prev) {
                    state[item.id] = { lastEpisode: latest, unread: false, checkedAt: new Date().toISOString() };
                } else if (prev.lastEpisode !== latest) {
                    state[item.id] = { lastEpisode: latest, unread: true, checkedAt: new Date().toISOString() };
                    try {
                        const { isPermissionGranted, requestPermission, sendNotification } = await import('@tauri-apps/plugin-notification');
                        let granted = await isPermissionGranted();
                        if (!granted) { const p = await requestPermission(); granted = p === 'granted'; }
                        if (granted) sendNotification({ title: item.title, body: `Nuevo episodio: Ep. ${latest}` });
                    } catch {}
                } else {
                    state[item.id] = { ...prev, checkedAt: new Date().toISOString() };
                }
                changed = true;
            }
        } catch (e) {
            console.error('Error checking anime notif for', item.id, e);
        }
    }

    if (changed) { saveAnimeNotif(state); animeNotifState.set(state); }
    checkingAnimeNotifs.set(false);
}

export function markAnimeNotifRead(animeId) {
    animeNotifState.update(s => {
        if (!s[animeId]) return s;
        const next = { ...s, [animeId]: { ...s[animeId], unread: false } };
        saveAnimeNotif(next);
        return next;
    });
}

export function markAllAnimeNotifsRead() {
    animeNotifState.update(s => {
        const next = {};
        for (const k in s) next[k] = { ...s[k], unread: false };
        saveAnimeNotif(next);
        return next;
    });
}

// ── Navegación desde biblioteca/notifs ────────────────────────────────────────
export const pendingAnime = writable(null);

export function openAnimeFromLibrary(item) {
    pendingAnime.set(item);
    currentView.set('anime');
}
