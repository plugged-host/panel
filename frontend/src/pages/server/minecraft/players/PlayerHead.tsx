// plugged.host: a player's head, cut from their skin by the panel (shared/src/minecraft/player_images.rs);
// the first letter of the name when it cannot be had (docs/features/minecraft.md#player-details)
import { useState } from 'react';
import { useServerStore } from '@/stores/server.ts';

export default function PlayerHead({ uuid, name, size }: { uuid: string; name?: string | null; size?: 'lg' }) {
  const server = useServerStore((state) => state.server);
  const [failed, setFailed] = useState(false);

  if (!uuid || failed) {
    return (
      <span className='theme-player-head theme-player-head-fallback' data-size={size} aria-hidden>
        {(name || '?').charAt(0)}
      </span>
    );
  }

  const query = name ? `?name=${encodeURIComponent(name)}` : '';

  return (
    <img
      src={`/api/client/servers/${server.uuid}/minecraft/players/${uuid}/head${query}`}
      alt=''
      loading='lazy'
      className='theme-player-head'
      data-size={size}
      onError={() => setFailed(true)}
    />
  );
}
