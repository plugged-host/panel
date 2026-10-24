// plugged.host: what neo's chat shares between its API calls, store and elements (docs/features/neo.md)

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
