// plugged.host: which ad goes into which slot of a page (docs/features/ads.md)
import { z } from 'zod';
import { adsZoneSchema } from '@/lib/schemas/settings.ts';
import { adBannerSchema } from '@/lib/schemas/user/ads.ts';

export type AdZone = z.infer<typeof adsZoneSchema>;
export type AdBanner = z.infer<typeof adBannerSchema>;

/** Where a slot sits; a lower rank picks first. */
export type AdSlotKind = 'top' | 'tools' | 'page' | 'footLeaderboard' | 'footRectangle' | 'native' | 'room';

const RANK: Record<AdSlotKind, number> = {
  top: 0,
  tools: 1,
  page: 2,
  footLeaderboard: 3,
  footRectangle: 3,
  native: 3,
  room: 4,
};

const LEADERBOARDS: [number, number][] = [
  [728, 90],
  [468, 60],
  [320, 50],
];

/** The sizes a slot takes; `null` takes any (the waiting rooms show what is left). */
const SIZES: Record<AdSlotKind, [number, number][] | null> = {
  top: LEADERBOARDS,
  tools: [
    [468, 60],
    [320, 50],
  ],
  page: [[300, 250]],
  footLeaderboard: LEADERBOARDS,
  footRectangle: [[300, 250]],
  native: [],
  room: null,
};

export type AdUnit =
  | { type: 'zone'; id: string; zone: AdZone; width: number; height: number }
  | { type: 'banner'; id: string; banner: AdBanner; width: number; height: number }
  | { type: 'native'; id: string };

export interface AdSlotState {
  id: number;
  kind: AdSlotKind;
  /** The width the slot has, in CSS pixels; 0 until measured. */
  width: number;
}

const ADSTERRA_SIZES = new Set(['728x90', '468x60', '320x50', '300x250', '160x600', '160x300']);

/** The size a slot banner is laid out at: an Adsterra size, or half of twice one (shared/src/models/ad_banner.rs). */
export function bannerSlotSize(width: number, height: number): [number, number] | null {
  if (ADSTERRA_SIZES.has(`${width}x${height}`)) return [width, height];
  if (width % 2 === 0 && height % 2 === 0 && ADSTERRA_SIZES.has(`${width / 2}x${height / 2}`)) {
    return [width / 2, height / 2];
  }
  return null;
}

/**
 * A copy shuffled by `seed` (the page's path), so banners of one size take turns from page to page
 * while every slot of a page sees the same order.
 */
export function shuffled<T>(items: T[], seed: string): T[] {
  // FNV-1a for the seed, then mulberry32
  let state = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    state = Math.imul(state ^ seed.charCodeAt(i), 0x01000193);
  }
  const random = () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };

  const copy = [...items];
  for (let i = copy.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [copy[i], copy[j]] = [copy[j], copy[i]];
  }
  return copy;
}

/** Dashboard or server pages, from the path. */
export function adAreaOf(pathname: string): 'account' | 'server' {
  return pathname.startsWith('/server/') ? 'server' : 'account';
}

/**
 * Gives each slot of a page at most one unit, no unit twice: slots pick by rank (then in the order
 * they appeared), each the widest unit of its sizes that fits; a sponsored banner goes before a
 * zone of its size, and a "top" banner takes the top slot.
 */
export function allocate(
  slots: AdSlotState[],
  zones: AdZone[],
  banners: AdBanner[],
  native: boolean,
): Map<number, AdUnit> {
  const units: AdUnit[] = [];
  for (const banner of banners) {
    if (banner.placement !== 'slot') continue;
    const size = bannerSlotSize(banner.width, banner.height);
    if (size) units.push({ type: 'banner', id: `banner:${banner.uuid}`, banner, width: size[0], height: size[1] });
  }
  for (const zone of zones) {
    units.push({ type: 'zone', id: `zone:${zone.key}`, zone, width: zone.width, height: zone.height });
  }
  // widest first; at one width the sponsored banner first (a stable sort keeps the rest)
  units.sort((a, b) => ('width' in b ? b.width : 0) - ('width' in a ? a.width : 0));

  const topBanners = banners.filter((banner) => banner.placement === 'top');
  const used = new Set<string>();
  const result = new Map<number, AdUnit>();

  const ordered = [...slots].sort((a, b) => RANK[a.kind] - RANK[b.kind] || a.id - b.id);
  for (const slot of ordered) {
    if (slot.width <= 0) continue;

    if (slot.kind === 'native') {
      if (native && !used.has('native')) {
        used.add('native');
        result.set(slot.id, { type: 'native', id: 'native' });
      }
      continue;
    }

    if (slot.kind === 'top') {
      const top = topBanners.find((banner) => !used.has(`top:${banner.uuid}`));
      if (top) {
        used.add(`top:${top.uuid}`);
        result.set(slot.id, {
          type: 'banner',
          id: `top:${top.uuid}`,
          banner: top,
          width: top.width,
          height: top.height,
        });
        continue;
      }
    }

    const sizes = SIZES[slot.kind];
    const unit = units.find(
      (unit) =>
        unit.type !== 'native' &&
        !used.has(unit.id) &&
        unit.width <= slot.width &&
        (sizes === null || sizes.some(([width, height]) => width === unit.width && height === unit.height)),
    );
    if (unit) {
      used.add(unit.id);
      result.set(slot.id, unit);
    }
  }

  return result;
}

/** The URL of a unit's frame document. */
export function unitFrameUrl(frameOrigin: string | null, unit: AdUnit): string {
  const query =
    unit.type === 'native' ? 'native=true' : unit.type === 'zone' ? `zone=${encodeURIComponent(unit.zone.key)}` : '';
  return `${frameOrigin ?? ''}/api/ads/unit?${query}`;
}

/** The sandbox of a unit frame: its own origin only on the ad frame address (shared/src/ads.rs `unit_sandbox`). */
export function unitSandbox(frameOrigin: string | null): string {
  return frameOrigin
    ? 'allow-scripts allow-same-origin allow-popups allow-popups-to-escape-sandbox allow-forms'
    : 'allow-scripts allow-popups allow-popups-to-escape-sandbox allow-forms';
}

export type ServerPageKind = 'console' | 'room' | 'page';

/** A server's console, its waiting rooms (start queue, waiting room) and the rest: where which ads go. */
export function serverPageKind(pathname: string): ServerPageKind {
  const rest = pathname.replace(/^\/server\/[^/]+/, '').replace(/\/+$/, '');
  if (rest === '') return 'console';
  if (rest === '/queue' || rest === '/waiting') return 'room';
  return 'page';
}

const storage = {
  get(key: string): string | null {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set(key: string, value: string) {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      // private mode: the notice and the Vignette then follow the page load only
    }
  },
};

/** Whether `minutes` have passed since `key` was stamped, and stamps it when they have. */
export function takeTurn(key: string, minutes: number): boolean {
  const now = Date.now();
  const last = Number(storage.get(key) ?? 0);
  if (minutes > 0 && now - last < minutes * 60_000) return false;
  storage.set(key, String(now));
  return true;
}

/** Whether a dismissal stamped at `key` still holds for `hours`. */
export function dismissed(key: string, hours: number): boolean {
  const at = Number(storage.get(key) ?? 0);
  return hours > 0 && Date.now() - at < hours * 3_600_000;
}

export function dismiss(key: string) {
  storage.set(key, String(Date.now()));
}
