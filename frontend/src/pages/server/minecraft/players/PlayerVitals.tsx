// plugged.host: a player's health, food and experience as the game shows them, with the console's
// Heal, Feed and Kill (docs/features/minecraft.md#player-details)
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import playerAction from '@/api/server/minecraft/playerAction.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { formatNumber } from '@/lib/format/serverUsage.ts';
import { minecraftPlayerActionSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { Food, Hearts } from './PixelIcons.tsx';
import { PlayerData, PlayerPage, playerBase } from './playerPage.ts';

type Action = z.infer<typeof minecraftPlayerActionSchema>;

export default function PlayerVitals({ page, data }: { page: PlayerPage; data: PlayerData }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canConsole = useServerCan('control.console');
  const [busy, setBusy] = useState<Action | null>(null);
  const [killing, setKilling] = useState(false);

  const message = (action: Action) => {
    switch (action) {
      case 'heal':
        return t(`${playerBase}.toast.healed`, { name: page.name });
      case 'feed':
        return t(`${playerBase}.toast.fed`, { name: page.name });
      default:
        return t(`${playerBase}.toast.killed`, { name: page.name });
    }
  };

  const act = (action: Action) => {
    setBusy(action);
    return playerAction(server.uuid, page.details.uuid, action)
      .then(() => {
        page.done(message(action), true);
        if (action === 'heal') page.patch((current) => ({ ...current, health: current.maxHealth }));
        if (action === 'feed') page.patch((current) => ({ ...current, food: 20 }));
        setKilling(false);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setBusy(null));
  };

  const progress = Math.round(Math.min(Math.max(data.xpProgress, 0), 1) * 100);

  return (
    <>
      <ConfirmationModal
        opened={killing}
        onClose={() => setKilling(false)}
        title={t(`${playerBase}.vitals.killTitle`, { name: page.name })}
        confirm={t(`${playerBase}.vitals.kill`, {})}
        onConfirmed={() => act('kill')}
      >
        <Text size='sm'>{t(`${playerBase}.vitals.killContent`, {})}</Text>
      </ConfirmationModal>

      <TitleCard
        title={t(`${playerBase}.vitals.title`, {})}
        footer={
          canConsole ? (
            <>
              {page.liveBlocked && <p className='theme-player-note me-auto self-center'>{page.liveBlocked}</p>}
              <div className='theme-player-actions'>
                <Button
                  variant='default'
                  size='sm'
                  disabled={!!page.liveBlocked}
                  loading={busy === 'heal'}
                  onClick={() => act('heal')}
                >
                  {t(`${playerBase}.vitals.heal`, {})}
                </Button>
                <Button
                  variant='default'
                  size='sm'
                  disabled={!!page.liveBlocked}
                  loading={busy === 'feed'}
                  onClick={() => act('feed')}
                >
                  {t(`${playerBase}.vitals.feed`, {})}
                </Button>
                <Button variant='default' size='sm' disabled={!!page.liveBlocked} onClick={() => setKilling(true)}>
                  {t(`${playerBase}.vitals.kill`, {})}
                </Button>
              </div>
            </>
          ) : undefined
        }
      >
        <div className='theme-player-vital'>
          <div className='theme-player-vital-head'>
            <span>{t(`${playerBase}.vitals.health`, {})}</span>
            <strong>
              {formatNumber(data.health, 1)} / {formatNumber(data.maxHealth, 1)}
            </strong>
          </div>
          <Hearts health={data.health} max={data.maxHealth} />
        </div>

        <div className='theme-player-vital'>
          <div className='theme-player-vital-head'>
            <span>{t(`${playerBase}.vitals.food`, {})}</span>
            <strong>{data.food} / 20</strong>
          </div>
          <Food food={data.food} />
        </div>

        <div className='theme-player-vital'>
          <div className='theme-player-vital-head'>
            <span>{t(`${playerBase}.vitals.level`, { level: data.xpLevel })}</span>
            <strong>{progress}%</strong>
          </div>
          <div
            className='theme-stat-track'
            role='progressbar'
            aria-label={t(`${playerBase}.vitals.level`, { level: data.xpLevel })}
            aria-valuenow={progress}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <span className='theme-stat-bar' style={{ width: `${progress}%` }} />
          </div>
          <p className='theme-player-note mt-2'>
            {t(`${playerBase}.vitals.points`, { points: formatNumber(data.xpTotal, 0) })}
          </p>
        </div>
      </TitleCard>
    </>
  );
}
