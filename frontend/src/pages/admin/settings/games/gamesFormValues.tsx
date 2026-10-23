// plugged.host: Admin › Settings › Games
import { UseFormReturnType } from '@mantine/form';
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { adminSettingsGamesSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import GameCatalogField from './GameCatalogField.tsx';

type GamesFormValues = z.infer<typeof adminSettingsGamesSchema>;

export const gamesEmptyFormValues: GamesFormValues = {
  createEnabled: false,
  catalog: [],
  limitAllocations: 1,
  limitDatabases: 0,
  limitBackups: 0,
  limitSchedules: 0,
  allowDeleting: false,
  maxServers: 0,
  allowanceCpu: 0,
  allowanceMemory: 0,
  allowanceDisk: 0,
};

export const gamesToFormValues = (games: GamesFormValues): Partial<GamesFormValues> => games;

export function useGamesFormFields(
  form: UseFormReturnType<GamesFormValues>,
  noprofitConnected: boolean,
): FieldDef<GamesFormValues>[] {
  const { t } = useTranslations();
  const page = 'pages.admin.settings.tabs.games.page.form';

  return [
    {
      type: 'switch',
      name: 'createEnabled',
      label: t(`${page}.createEnabled`, {}),
      description: t(`${page}.createEnabledDescription`, {}),
    },
    {
      type: 'switch',
      name: 'allowDeleting',
      label: t(`${page}.allowDeleting`, {}),
      description: t(`${page}.allowDeletingDescription`, {}),
    },
    {
      type: 'number',
      name: 'maxServers',
      label: t(`${page}.maxServers`, {}),
      description: t(`${page}.maxServersDescription`, {}),
      required: true,
      props: { min: 0, max: 1000 },
    },
    {
      type: 'divider',
      name: 'limitsDivider',
      label: t(`${page}.limits`, {}),
    },
    {
      type: 'number',
      name: 'limitAllocations',
      label: t(`${page}.limitAllocations`, {}),
      required: true,
      props: { min: 1, max: 100 },
    },
    {
      type: 'number',
      name: 'limitDatabases',
      label: t(`${page}.limitDatabases`, {}),
      required: true,
      props: { min: 0, max: 100 },
    },
    {
      type: 'number',
      name: 'limitBackups',
      label: t(`${page}.limitBackups`, {}),
      required: true,
      props: { min: 0, max: 100 },
    },
    {
      type: 'number',
      name: 'limitSchedules',
      label: t(`${page}.limitSchedules`, {}),
      required: true,
      props: { min: 0, max: 100 },
    },
    {
      type: 'divider',
      name: 'allowanceDivider',
      label: t(`${page}.allowance`, {}),
    },
    {
      type: 'number',
      name: 'allowanceCpu',
      label: t(`${page}.allowanceCpu`, {}),
      description: noprofitConnected ? t(`${page}.allowanceNoprofit`, {}) : t(`${page}.allowanceDescription`, {}),
      required: true,
      props: { min: 0 },
    },
    { type: 'size', name: 'allowanceMemory', label: t('common.form.memory', {}), required: true, mode: 'mb', min: 0 },
    { type: 'size', name: 'allowanceDisk', label: t('common.form.disk', {}), required: true, mode: 'mb', min: 0 },
    {
      type: 'divider',
      name: 'catalogDivider',
      label: t(`${page}.catalog`, {}),
    },
    {
      type: 'custom',
      name: 'catalog',
      colSpan: 'full',
      render: () => <GameCatalogField form={form} />,
    },
  ];
}
