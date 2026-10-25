// plugged.host: a player's inventory as the game lays it out: armor and off hand, the 27 slots, the
// hotbar, then the ender chest (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import { minecraftItemSchema } from '@/lib/schemas/server/minecraft.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import ItemSlot from './ItemSlot.tsx';
import { PlayerData, playerBase } from './playerPage.ts';

type Item = z.infer<typeof minecraftItemSchema>;

const range = (from: number, count: number) => Array.from({ length: count }, (_, i) => from + i);

function Slots({ items, slots, version }: { items: Map<number, Item>; slots: number[]; version: string }) {
  return (
    <div className='theme-player-slots'>
      {slots.map((slot) => (
        <ItemSlot key={slot} item={items.get(slot)} version={version} />
      ))}
    </div>
  );
}

export default function PlayerInventory({ data, version }: { data: PlayerData; version: string }) {
  const { t } = useTranslations();

  const inventory = new Map(data.inventory.map((item) => [item.slot, item]));
  const enderChest = new Map(data.enderChest.map((item) => [item.slot, item]));

  return (
    <TitleCard title={t(`${playerBase}.inventory.title`, {})}>
      <section className='theme-player-section'>
        <h3 className='theme-player-caption'>{t(`${playerBase}.inventory.equipment`, {})}</h3>
        <div className='theme-player-slots'>
          {/* head to feet, then the off hand */}
          {[103, 102, 101, 100].map((slot) => (
            <ItemSlot key={slot} item={inventory.get(slot)} version={version} />
          ))}
          <ItemSlot item={inventory.get(-106)} version={version} className='theme-player-offhand' />
        </div>
      </section>

      <section className='theme-player-section'>
        <h3 className='theme-player-caption'>{t(`${playerBase}.inventory.main`, {})}</h3>
        <Slots items={inventory} slots={range(9, 27)} version={version} />
      </section>

      <section className='theme-player-section'>
        <h3 className='theme-player-caption'>{t(`${playerBase}.inventory.hotbar`, {})}</h3>
        <Slots items={inventory} slots={range(0, 9)} version={version} />
      </section>

      <section className='theme-player-section'>
        <h3 className='theme-player-caption'>{t(`${playerBase}.inventory.enderChest`, {})}</h3>
        <Slots items={enderChest} slots={range(0, 27)} version={version} />
      </section>
    </TitleCard>
  );
}
