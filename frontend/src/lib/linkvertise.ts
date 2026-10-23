// plugged.host: Linkvertise's Full Script on the Tokens page (docs/features/economy.md). The script
// rewrites the page's links that match the whitelist into Linkvertise links, at DOMContentLoaded,
// which in the panel has long passed: the listener it registers is caught and run right away.

const SCRIPT_URL = 'https://publisher.linkvertise.com/cdn/linkvertise.js';

type Linkvertise = (publisher: number, options: { whitelist: string[]; blacklist: string[] }) => void;

let loading: Promise<void> | null = null;

function loadScript(): Promise<void> {
  if (loading) return loading;

  loading = new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = SCRIPT_URL;
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => {
      loading = null;
      script.remove();
      reject(new Error('linkvertise.js did not load'));
    };
    document.head.appendChild(script);
  });

  return loading;
}

// Rewrites the links on the page now; call it after they are rendered.
export async function armFullScript(publisher: number, whitelist: string[]): Promise<void> {
  await loadScript();

  const linkvertise = (window as unknown as { linkvertise?: Linkvertise }).linkvertise;
  if (!linkvertise) return;

  const original = document.addEventListener;
  const caught: EventListenerOrEventListenerObject[] = [];

  document.addEventListener = ((
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ) => {
    if (type === 'DOMContentLoaded') {
      caught.push(listener);
      return;
    }

    original.call(document, type, listener, options);
  }) as typeof document.addEventListener;

  try {
    linkvertise(publisher, { whitelist, blacklist: [] });
  } finally {
    document.addEventListener = original;
  }

  const event = new Event('DOMContentLoaded');
  for (const listener of caught) {
    if (typeof listener === 'function') {
      listener.call(document, event);
    } else {
      listener.handleEvent(event);
    }
  }
}
