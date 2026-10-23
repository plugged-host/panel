import { ReactNode } from 'react';
import { z } from 'zod';
import Switch from '@/elements/input/Switch.tsx';
import TrashRetentionInput from '@/elements/input/TrashRetentionInput.tsx';
import TrashUsageThresholdInput from '@/elements/input/TrashUsageThresholdInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { formatTrashSettings } from '@/lib/domain/server.ts';
import { serverTrashSettingsSchema } from '@/lib/schemas/server/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type TrashSettings = z.infer<typeof serverTrashSettingsSchema>;

export default function TrashSettingsInput({
  value,
  defaults,
  onChange,
  errors,
}: {
  value: TrashSettings | null;
  defaults: TrashSettings;
  onChange: (value: TrashSettings | null) => void;
  errors?: { retention?: ReactNode; usageThreshold?: ReactNode };
}) {
  const { t } = useTranslations();

  return (
    <Stack gap='xs'>
      <Switch
        label={t('common.elements.trashSettings.custom', {})}
        description={
          value ? undefined : t('common.elements.trashSettings.default', { summary: formatTrashSettings(defaults) })
        }
        checked={value !== null}
        onChange={(e) => onChange(e.currentTarget.checked ? defaults : null)}
      />
      {value && (
        <>
          <TrashRetentionInput
            label={t('common.elements.trashSettings.retention', {})}
            description={t('common.elements.trashSettings.retentionDescription', {})}
            value={value.retention}
            onChange={(retention) => onChange({ ...value, retention })}
            error={errors?.retention}
          />
          <TrashUsageThresholdInput
            label={t('common.elements.trashSettings.usageThreshold', {})}
            description={t('common.elements.trashSettings.usageThresholdDescription', {})}
            value={value.usageThreshold}
            onChange={(usageThreshold) => onChange({ ...value, usageThreshold })}
            error={errors?.usageThreshold}
          />
        </>
      )}
    </Stack>
  );
}
