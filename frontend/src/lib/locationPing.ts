// plugged.host: a location's ping as the visitor sees it. The panel's own figure (`latency`) is its
// distance to the nodes, measured from wherever the panel runs: from Paris, Thailand read 763 ms to
// everyone. Here the browser times small requests to a location's nodes (`probes`, their Wings
// addresses): the first request also opens the connection (DNS, TCP, TLS) and is left out, the
// fastest of the next three is about one round trip. A location's ping is its fastest node's. Each
// node is timed once per page load, so a re-render or another list does not ask again. The answer
// is opaque (no-cors): only the time is read, never the response.
import { useEffect, useState } from 'react';

const nodes = new Map<string, Promise<number | null>>();

async function once(url: string): Promise<number | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5000);
  const started = performance.now();

  try {
    await fetch(url, { mode: 'no-cors', cache: 'no-store', credentials: 'omit', signal: controller.signal });

    return performance.now() - started;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

async function node(url: string): Promise<number | null> {
  if ((await once(url)) === null) return null;

  const times: number[] = [];
  for (let i = 0; i < 3; i++) {
    const ms = await once(url);
    if (ms !== null) times.push(ms);
  }

  return times.length > 0 ? Math.max(1, Math.round(Math.min(...times))) : null;
}

/** The fastest node's ping in ms, or null when none answered. */
export async function measureLocation(urls: string[]): Promise<number | null> {
  const times = await Promise.all(
    urls.map((url) => {
      let ping = nodes.get(url);
      if (!ping) {
        ping = node(url);
        nodes.set(url, ping);
      }

      return ping;
    }),
  );
  const answered = times.filter((ms): ms is number => ms !== null);

  return answered.length > 0 ? Math.min(...answered) : null;
}

/** The visitor's ping per location uuid, filled in as the locations' nodes answer. */
export function useLocationPings(locations: { uuid: string; probes: string[] }[]): Record<string, number> {
  const [pings, setPings] = useState<Record<string, number>>({});
  const wanted = JSON.stringify(locations.map((location) => [location.uuid, location.probes]));

  useEffect(() => {
    let alive = true;

    for (const [uuid, probes] of JSON.parse(wanted) as [string, string[]][]) {
      if (probes.length === 0) continue;

      measureLocation(probes).then((ms) => {
        if (alive && ms !== null) {
          setPings((current) => (current[uuid] === ms ? current : { ...current, [uuid]: ms }));
        }
      });
    }

    return () => {
      alive = false;
    };
  }, [wanted]);

  return pings;
}

/** The closest location that can take the server now, or null before any has a ping. */
export function closestLocation(
  locations: { uuid: string; unlocked: boolean; available: boolean }[],
  pings: Record<string, number>,
): string | null {
  let best: { uuid: string; ms: number } | null = null;

  for (const location of locations) {
    const ms = pings[location.uuid];
    if (ms === undefined || !location.unlocked || !location.available) continue;
    if (best === null || ms < best.ms) best = { uuid: location.uuid, ms };
  }

  return best?.uuid ?? null;
}
