// plugged.host: one inventory slot: the item's icon (through the panel, shared/src/minecraft/player_images.rs)
// with its count, its name and enchantments on hover or tap; a mod's item or one without an icon
// shows the start of its name instead (docs/features/minecraft.md#player-details)
import { useState } from 'react';
import { z } from 'zod';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { minecraftItemSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerStore } from '@/stores/server.ts';

type Item = z.infer<typeof minecraftItemSchema>;

const VERSION = /^[a-z0-9][a-z0-9._-]{0,31}$/;

function initials(label: string): string {
  return label
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word.charAt(0))
    .join('');
}

export default function ItemSlot({ item, version, className }: { item?: Item; version: string; className?: string }) {
  const server = useServerStore((state) => state.server);
  const [failed, setFailed] = useState(false);

  if (!item) {
    return (
      <div className={className ? `theme-player-cell ${className}` : 'theme-player-cell'}>
        <span className='theme-player-slot' />
      </div>
    );
  }

  const name = item.customName || item.label;
  const gameVersion = VERSION.test(version) ? version : 'latest';
  const icon = item.id.startsWith('minecraft:') && !failed;

  return (
    <div className={className ? `theme-player-cell ${className}` : 'theme-player-cell'}>
      <Tooltip
        multiline
        maw={260}
        events={{ hover: true, focus: true, touch: true }}
        label={
          <span className='theme-player-tip'>
            <strong>{name}</strong>
            {item.customName && <span className='theme-player-tip-muted'>{item.label}</span>}
            {item.enchantments.map((enchantment) => (
              <span key={enchantment.id}>{enchantment.label}</span>
            ))}
            <span className='theme-player-tip-muted' dir='ltr'>
              {item.id}
            </span>
          </span>
        }
      >
        <span
          className='theme-player-slot'
          // focusable, so the tooltip with the item's name opens from the keyboard too
          tabIndex={0}
          role='img'
          aria-label={item.count > 1 ? `${name} ×${item.count}` : name}
          data-enchanted={item.enchantments.length > 0 || undefined}
        >
          {icon ? (
            <img
              src={`/api/client/servers/${server.uuid}/minecraft/players/item-icon?id=${encodeURIComponent(item.id)}&version=${encodeURIComponent(gameVersion)}`}
              alt=''
              loading='lazy'
              onError={() => setFailed(true)}
            />
          ) : (
            <span className='theme-player-slot-text'>{initials(item.label)}</span>
          )}
          {item.count > 1 && <span className='theme-player-slot-count'>{item.count}</span>}
        </span>
      </Tooltip>
    </div>
  );
}
