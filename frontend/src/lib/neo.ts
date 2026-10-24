// plugged.host: what neo's chat shares between its API calls, store and elements, and the helpers of
// its settings page (Admin › Settings › AI › neo) (docs/features/neo.md)

/** The chat routes of a place: a server's chat, or the account's chat when `serverUuid` is null. */
export function neoBase(serverUuid: string | null): string {
  return serverUuid ? `/api/client/servers/${serverUuid}/neo` : '/api/client/account/neo';
}

/** An image of one of the person's chats (any place), as JPEG; the session cookie authorizes it. */
export function neoImageUrl(uuid: string): string {
  return `/api/client/account/neo/images/${uuid}`;
}

/** What the backend reads (images.rs): anything else is refused before it is sent. */
export const NEO_IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];

// the longest side the models read without scaling the image down themselves
const MAX_SIDE = 1568;
const QUALITY = 0.85;

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('image could not be read'));
    image.src = url;
  });
}

/**
 * A picked, pasted or dropped image as a JPEG data URL with its longest side at most 1568 px, so a
 * phone photo goes up as a few hundred KB. Transparent parts become white (JPEG has no alpha), a GIF
 * keeps its first frame.
 */
export async function shrinkNeoImage(file: File): Promise<string> {
  const url = URL.createObjectURL(file);

  try {
    const image = await loadImage(url);
    const scale = Math.min(1, MAX_SIDE / Math.max(image.naturalWidth, image.naturalHeight, 1));
    const width = Math.max(1, Math.round(image.naturalWidth * scale));
    const height = Math.max(1, Math.round(image.naturalHeight * scale));

    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;

    const context = canvas.getContext('2d');
    if (!context) {
      throw new Error('image could not be read');
    }

    context.fillStyle = '#ffffff';
    context.fillRect(0, 0, width, height);
    context.drawImage(image, 0, 0, width, height);

    return canvas.toDataURL('image/jpeg', QUALITY);
  } finally {
    URL.revokeObjectURL(url);
  }
}

// settings page

/** Model slots a panel can fill (`MAX_PROVIDERS` in shared/src/settings/neo.rs). */
export const NEO_MAX_PROVIDERS = 6;

/** What the panel shows a secret extra header's value as; sent back unchanged, it keeps the value. */
export const NEO_HEADER_MASK = '••••';

/** A JSON object typed into a text field: `{}` when empty, `null` when it is not an object. */
export function parseJsonObject(text: string): Record<string, unknown> | null {
  if (!text.trim()) {
    return {};
  }

  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === 'object' && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

/** Extra headers typed as JSON: an object of text values, or `null`. */
export function parseJsonHeaders(text: string): Record<string, string> | null {
  const headers = parseJsonObject(text);
  if (!headers || !Object.values(headers).every((value) => typeof value === 'string')) {
    return null;
  }

  return headers as Record<string, string>;
}

/** A stored object as the text field shows it: empty for none. */
export function formatJsonObject(value: Record<string, unknown>): string {
  return Object.keys(value).length ? JSON.stringify(value, null, 2) : '';
}

/** The lowest slot number not taken yet, or `null` when every slot is. */
export function nextFreeSlot(taken: number[]): number | null {
  for (let slot = 1; slot <= NEO_MAX_PROVIDERS; slot++) {
    if (!taken.includes(slot)) {
      return slot;
    }
  }

  return null;
}

/**
 * Whether noprofit.space runs neo instead of this panel's models: it is connected
 * (`AppSettingsNoprofit::config`) and has neo's gateway secret (`neo::on_noprofit`).
 */
export function neoOnNoprofit(noprofit: {
  enabled: boolean;
  accountsUrl: string;
  ssoClient: string;
  ssoSecret: string | null;
  neoSecret: string | null;
}): boolean {
  return (
    noprofit.enabled &&
    noprofit.accountsUrl.trim() !== '' &&
    noprofit.ssoClient.trim() !== '' &&
    noprofit.ssoSecret !== null &&
    noprofit.neoSecret !== null
  );
}
