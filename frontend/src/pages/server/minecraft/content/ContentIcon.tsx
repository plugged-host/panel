// plugged.host: a catalog icon through the panel's own image endpoint. Some browsers and privacy
// settings refuse third-party images, and some CDNs refuse hotlinks; the panel fetches and caches
// the picture instead, so a plugin or mod always has one. A broken or missing icon falls back to
// the puzzle/box glyph instead of an empty hole.
import { faBox, faPuzzlePiece } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import { minecraftKindSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerStore } from '@/stores/server.ts';

type Kind = z.infer<typeof minecraftKindSchema> | null;

function iconUrl(serverUuid: string, icon: string): string {
  if (icon.startsWith('data:') || icon.startsWith('blob:') || icon.startsWith('/')) {
    return icon;
  }
  if (!/^https?:\/\//i.test(icon)) {
    return icon;
  }

  return `/api/client/servers/${serverUuid}/minecraft/content/icon?url=${encodeURIComponent(icon)}`;
}

export default function ContentIcon({
  icon,
  kind,
  variant = 'catalog',
}: {
  icon: string | null;
  kind: Kind;
  variant?: 'catalog' | 'jar';
}) {
  const server = useServerStore((state) => state.server);
  const [failed, setFailed] = useState(false);

  const fallback = (
    <FontAwesomeIcon icon={kind === 'mod' ? faBox : faPuzzlePiece} size={variant === 'jar' ? 'lg' : undefined} />
  );

  if (!icon || failed) {
    return variant === 'catalog' ? (
      <span className='theme-catalog-icon theme-catalog-icon-fallback'>{fallback}</span>
    ) : (
      fallback
    );
  }

  return (
    <img
      src={iconUrl(server.uuid, icon)}
      alt=''
      loading='lazy'
      className={variant === 'catalog' ? 'theme-catalog-icon' : undefined}
      onError={() => setFailed(true)}
    />
  );
}
