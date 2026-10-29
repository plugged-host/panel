// plugged.host: the server's upgrades, shared by the Settings section and the offers where a limit or limbo
// gets in the way, and what a purchase refreshes (docs/features/server-upgrades.md)
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import getServer from '@/api/server/getServer.ts';
import getServerUpgrades from '@/api/server/upgrades/getServerUpgrades.ts';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverUpgradeKindSchema, serverUpgradesSchema } from '@/lib/schemas/server/upgrades.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export type Upgrades = z.infer<typeof serverUpgradesSchema>;
export type UpgradeKind = z.infer<typeof serverUpgradeKindSchema>;
export type UpgradeOffer = Upgrades['offers'][number];

/** A purchase about to be confirmed: a new one, or one that runs made longer. */
export interface UpgradeRequest {
  kind: UpgradeKind;
  renew?: string | null;
  color?: string | null;
}

export const upgradesBase = 'pages.server.settings.upgrades';

/** The translation key of a kind (`keep_awake` → `keepAwake`). */
export const KIND_KEYS = {
  keep_awake: 'keepAwake',
  boost: 'boost',
  extra_allocation: 'extraAllocation',
  extra_database: 'extraDatabase',
  extra_backup: 'extraBackup',
  card_color: 'cardColor',
} as const;

export function useServerUpgrades(enabled = true) {
  const server = useServerStore((state) => state.server);
  const canUpgrade = useServerCan('settings.upgrades');

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.upgrades(),
    queryFn: () => getServerUpgrades(server.uuid),
    enabled: canUpgrade && enabled,
    silent: true,
  });

  return { resource, canUpgrade };
}

/** "24 hours", "7 days", "a day": a boost's hours stay hours. */
export function useUpgradeDuration() {
  const { t } = useTranslations();

  return (hours: number | null) => {
    if (hours === null) return '';
    if (hours === 1) return t(`${upgradesBase}.duration.hour`, {});
    if (hours < 48 || hours % 24 !== 0) return t(`${upgradesBase}.duration.hours`, { count: hours });
    if (hours === 24) return t(`${upgradesBase}.duration.day`, {});

    return t(`${upgradesBase}.duration.days`, { count: hours / 24 });
  };
}

/** What an offer does, for its row and its confirmation: "Limbo won't stop or park this server … for 7 days." */
export function useUpgradeDescription() {
  const { t } = useTranslations();
  const duration = useUpgradeDuration();

  return (offer: UpgradeOffer) => {
    const span = duration(offer.hours);

    switch (offer.kind) {
      case 'keep_awake':
        return t(`${upgradesBase}.kinds.keepAwake.body`, { duration: span });
      case 'boost':
        return t(`${upgradesBase}.kinds.boost.body`, {
          memory: bytesToString(mbToBytes(offer.memory)),
          cpu: offer.cpu,
          duration: span,
        });
      case 'extra_allocation':
        return t(`${upgradesBase}.kinds.extraAllocation.body`, { duration: span });
      case 'extra_database':
        return t(`${upgradesBase}.kinds.extraDatabase.body`, { duration: span });
      case 'extra_backup':
        return t(`${upgradesBase}.kinds.extraBackup.body`, { duration: span });
      case 'card_color':
        return t(`${upgradesBase}.kinds.cardColor.body`, {});
    }
  };
}

/** After a purchase: the section's answer, the sidebar balance, and the server's limits and card colour. */
export function useUpgradeRefresh() {
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);

  return (upgrades: Upgrades) => {
    queryClient.setQueryData(queryKeys.server(server.uuid).settings.upgrades(), upgrades);
    queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
    getServer(server.uuid)
      .then((fresh) =>
        updateServer({
          limits: fresh.limits,
          featureLimits: fresh.featureLimits,
          cardColor: fresh.cardColor,
          limboState: fresh.limboState,
        }),
      )
      .catch(() => {
        // the page shows the old limits until it is loaded again
      });
  };
}
