// plugged.host: the API answers the server put into the page's HTML (backend/src/prefetch.rs,
// docs/features/speed.md). Each one answers the first request for its URL, once, within a short
// time after the page loaded; anything later asks the server as usual.

const FRESH_FOR_MS = 15_000;

const loadedAt = Date.now();
const answers = new Map<string, unknown>();

try {
  const element = document.getElementById('prefetched');
  if (element?.textContent) {
    for (const [url, answer] of Object.entries(JSON.parse(element.textContent) as Record<string, unknown>)) {
      answers.set(url, answer);
    }
  }
  element?.remove();
} catch {
  answers.clear();
}

/** The URL as the server names its answers: path, then the non-empty query parameters sorted by name. */
export function normalizedUrl(url: string): string {
  const parsed = new URL(url, window.location.origin);
  const params = [...parsed.searchParams.entries()]
    .filter(([, value]) => value !== '')
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));

  return params.length > 0 ? `${parsed.pathname}?${new URLSearchParams(params).toString()}` : parsed.pathname;
}

/** The answer for `url` if the page brought one along and it is still fresh; it is used up either way. */
export function takePrefetched(url: string): { found: true; data: unknown } | { found: false } {
  if (answers.size === 0) return { found: false };

  const key = normalizedUrl(url);
  if (!answers.has(key)) return { found: false };

  const data = answers.get(key);
  answers.delete(key);

  return Date.now() - loadedAt <= FRESH_FOR_MS ? { found: true, data } : { found: false };
}

/** Drops every answer (the tab now acts as someone else). */
export function forgetPrefetched() {
  answers.clear();
}
