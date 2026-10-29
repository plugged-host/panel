// plugged.host: an upgrade offered where its lack gets in the way: one more port, database or backup slot
// once the limit is reached, a keep-awake pass once limbo stopped the server for being idle
// (docs/features/server-upgrades.md)
import { useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Notification from '@/elements/feedback/Notification.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import UpgradeConfirmModal from './UpgradeConfirmModal.tsx';
import { upgradesBase, useServerUpgrades, useUpgradeDuration } from './useServerUpgrades.ts';

const OFFERS = {
  extra_allocation: 'extraAllocation',
  extra_database: 'extraDatabase',
  extra_backup: 'extraBackup',
  keep_awake: 'keepAwake',
} as const;

export default function UpgradeOfferCallout({
  kind,
  show,
  text,
  className = 'mb-4',
}: {
  kind: keyof typeof OFFERS;
  /** The limit is reached, or the server was stopped for being idle. */
  show: boolean;
  /** Other words for the offer: a parked server's keep-awake pass. */
  text?: 'keepAwakeParked';
  className?: string;
}) {
  const { t, tItem } = useTranslations();
  const duration = useUpgradeDuration();
  const { resource } = useServerUpgrades(show);
  const [confirming, setConfirming] = useState(false);

  const upgrades = resource.data;
  const offer = upgrades?.offers.find((candidate) => candidate.kind === kind);
  // a keep-awake pass that runs already keeps it awake
  const running = kind === 'keep_awake' && !!upgrades?.purchases.some((purchase) => purchase.kind === kind);

  if (!show || !upgrades || !offer || offer.unavailable !== null || running) return null;

  const key = text ?? OFFERS[kind];
  const price = tItem('token', offer.tokens);

  return (
    <>
      {confirming && (
        <UpgradeConfirmModal upgrades={upgrades} request={{ kind }} onClose={() => setConfirming(false)} />
      )}

      <Notification className={`theme-callout ${className}`} title={t(`${upgradesBase}.offer.${key}.title`, {})}>
        <Stack gap='sm' align='flex-start'>
          <span>{t(`${upgradesBase}.offer.${key}.body`, { duration: duration(offer.hours), price })}</span>
          <Button size='xs' variant='default' onClick={() => setConfirming(true)}>
            {t(`${upgradesBase}.offer.${key}.button`, { price })}
          </Button>
        </Stack>
      </Notification>
    </>
  );
}
