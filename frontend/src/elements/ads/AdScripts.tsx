// plugged.host: the ad scripts that run in the page itself (docs/features/ads.md#page-level-scripts)
import { useEffect } from 'react';
import { takeTurn } from '@/lib/ads.ts';
import { type AdArea, useAds } from '@/plugins/useAds.ts';

const loaded = new Set<string>();

/** Whether a page-level ad script runs in this page load (only a full load ends it). */
export function adScriptsLoaded(): boolean {
  return loaded.size > 0;
}

function load(src: string, zone?: string) {
  if (loaded.has(src)) return;
  loaded.add(src);

  const script = document.createElement('script');
  script.src = src;
  script.async = true;
  script.dataset.cfasync = 'false';
  if (zone) script.dataset.zone = zone;
  document.body.appendChild(script);
}

/**
 * `page` loads the Social Bar (when it goes everywhere) and Monetag's tags, `room` the waiting
 * rooms' own; `none` (the console, a parked server) loads none. Nothing unloads before the next
 * full page load.
 */
export default function AdScripts({ area, kind }: { area: AdArea; kind: 'page' | 'room' | 'none' }) {
  const { settings, account, shown, pageShown } = useAds(area);

  useEffect(() => {
    if (!shown || kind === 'none') return;
    // the first membership: no popups or page scripts, not even in the waiting room
    if (!pageShown) return;

    if (kind === 'room') {
      if (settings.roomSocialBar && settings.socialBarScript) load(settings.socialBarScript);
      if (settings.roomPopunder && settings.popunderScript) load(settings.popunderScript);
      if (settings.roomInterstitial && settings.roomScript) load(settings.roomScript);
      return;
    }

    if (settings.socialBarEverywhere && settings.socialBarScript) load(settings.socialBarScript);
    if (settings.monetagPush) load(settings.monetagPush.src, settings.monetagPush.zone);

    const vignette = settings.monetagVignette;
    if (
      vignette &&
      !loaded.has(vignette.src) &&
      (account?.audience === 'everyone' || settings.monetagVignetteMembers) &&
      takeTurn('ads:vignette', settings.monetagVignetteMinutes)
    ) {
      load(vignette.src, vignette.zone);
    }
  }, [shown, pageShown, kind, settings, account?.audience]);

  return null;
}
