// plugged.host: the Behaviour page as one section of the Pelican panel's settings page: Calagopus' own
// auto-start, timezone, auto-kill and trash settings (Pelican has none of them) laid out like its
// Resources section, fields in Filament's grid and a toggle with the fields it opens under it, and
// one Save in the section's footer that sends only the parts that changed, each to its own route
// (docs/features/server-settings.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { FormEvent, useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import updateAutokill from '@/api/server/settings/updateAutokill.ts';
import updateAutostart from '@/api/server/settings/updateAutostart.ts';
import updateTimezone from '@/api/server/settings/updateTimezone.ts';
import updateTrash from '@/api/server/settings/updateTrash.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TimezoneInput from '@/elements/input/TimezoneInput.tsx';
import TrashRetentionInput from '@/elements/input/TrashRetentionInput.tsx';
import TrashUsageThresholdInput from '@/elements/input/TrashUsageThresholdInput.tsx';
import { formatTrashSettings } from '@/lib/domain/server.ts';
import { formatDateTimeAsTimezone } from '@/lib/format/time.ts';
import {
  serverSettingsAutokillSchema,
  serverSettingsAutostartSchema,
  serverSettingsTimezoneSchema,
  serverSettingsTrashSchema,
} from '@/lib/schemas/server/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

const base = 'pages.server.settings';
// Filament's helper text sits under the field
const HELPER_UNDER: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

// the server's time in the chosen timezone, ticking, as the timezone field's helper text
function useTimeIn(timezone: string | null) {
  const [time, setTime] = useState<string | null>(null);

  useEffect(() => {
    if (!timezone) {
      setTime(null);
      return;
    }

    setTime(formatDateTimeAsTimezone(new Date(), timezone, true));
    const interval = setInterval(() => setTime(formatDateTimeAsTimezone(new Date(), timezone, true)), 1000);

    return () => clearInterval(interval);
  }, [timezone]);

  return time;
}

export default function BehaviourContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const allowChangingTrashSettings = useGlobalStore((state) => state.settings.server.allowChangingTrashSettings);

  const canAutostart = useServerCan('settings.auto-start');
  const canTimezone = useServerCan('settings.timezone');
  const canAutokill = useServerCan('settings.auto-kill');
  const canTrash = useServerCan('settings.trash') && allowChangingTrashSettings;

  const [saving, setSaving] = useState(false);

  const autostart = useForm<z.infer<typeof serverSettingsAutostartSchema>>({
    initialValues: { behavior: server.autoStartBehavior },
    ...liveValidation,
    validate: zod4Resolver(serverSettingsAutostartSchema),
  });
  const timezone = useForm<z.infer<typeof serverSettingsTimezoneSchema>>({
    initialValues: { timezone: server.timezone ?? null },
    ...liveValidation,
    validate: zod4Resolver(serverSettingsTimezoneSchema),
  });
  const autokill = useForm<z.infer<typeof serverSettingsAutokillSchema>>({
    initialValues: { enabled: server.autoKill.enabled, seconds: server.autoKill.seconds },
    ...liveValidation,
    validate: zod4Resolver(serverSettingsAutokillSchema),
  });
  const trash = useForm<z.infer<typeof serverSettingsTrashSchema>>({
    initialValues: {
      custom: server.trash.custom
        ? { retention: server.trash.retention ?? 0, usageThreshold: server.trash.usageThreshold ?? 0 }
        : null,
    },
    ...liveValidation,
    validate: zod4Resolver(serverSettingsTrashSchema),
  });

  const time = useTimeIn(timezone.values.timezone);

  // each part saves to its own route, with its own toast, as upstream's four cards did
  const parts = [
    {
      shown: canAutostart,
      dirty: autostart.isDirty(),
      valid: autostart.isValid(),
      save: () => {
        const values = autostart.getValues();
        return updateAutostart(server.uuid, values).then(() => {
          addToast(t(`${base}.autostart.toast.updated`, {}), 'success');
          updateServer({ autoStartBehavior: values.behavior });
          autostart.resetDirty(values);
        });
      },
    },
    {
      shown: canTimezone,
      dirty: timezone.isDirty(),
      valid: timezone.isValid(),
      save: () => {
        const values = timezone.getValues();
        return updateTimezone(server.uuid, values).then(() => {
          addToast(t(`${base}.timezone.toast.updated`, {}), 'success');
          updateServer({ timezone: values.timezone });
          timezone.resetDirty(values);
        });
      },
    },
    {
      shown: canAutokill,
      dirty: autokill.isDirty(),
      valid: autokill.isValid(),
      save: () => {
        const values = autokill.getValues();
        return updateAutokill(server.uuid, values).then(() => {
          addToast(t(`${base}.autokill.toast.updated`, {}), 'success');
          updateServer({ autoKill: values });
          autokill.resetDirty(values);
        });
      },
    },
    {
      shown: canTrash,
      dirty: trash.isDirty(),
      valid: trash.isValid(),
      save: () => {
        const values = trash.getValues();
        return updateTrash(server.uuid, values).then(() => {
          addToast(t(`${base}.trash.toast.updated`, {}), 'success');

          const effective = values.custom ?? server.trash.defaults;
          updateServer({
            trash: {
              ...server.trash,
              custom: values.custom !== null,
              retention: effective.retention === 0 ? null : effective.retention,
              usageThreshold: effective.usageThreshold === 0 ? null : effective.usageThreshold,
            },
          });
          trash.resetDirty(values);
        });
      },
    },
  ];

  const shown = parts.filter((part) => part.shown);
  const changed = shown.filter((part) => part.dirty);
  const valid = changed.every((part) => part.valid);

  if (shown.length === 0) {
    return null;
  }

  const save = (event?: FormEvent) => {
    event?.preventDefault();
    if (saving || changed.length === 0 || !valid) return;

    setSaving(true);
    Promise.allSettled(
      changed.map((part) => part.save().catch((err) => addToast(httpErrorToHuman(err), 'error'))),
    ).finally(() => setSaving(false));
  };

  const custom = trash.values.custom;

  return (
    <form onSubmit={save}>
      <TitleCard
        title={t(`${base}.nav.behaviour`, {})}
        footer={
          <Button type='submit' loading={saving} disabled={changed.length === 0 || !valid}>
            {t('common.button.save', {})}
          </Button>
        }
      >
        <div className='flex flex-col gap-6'>
          {(canAutostart || canTimezone) && (
            <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
              {canAutostart && (
                <Select
                  withAsterisk
                  label={t(`${base}.autostart.title`, {})}
                  data={[
                    { label: t('common.enum.serverAutoStartBehavior.always', {}), value: 'always' },
                    { label: t('common.enum.serverAutoStartBehavior.unlessStopped', {}), value: 'unless_stopped' },
                    { label: t('common.enum.serverAutoStartBehavior.never', {}), value: 'never' },
                  ]}
                  {...autostart.getInputProps('behavior')}
                />
              )}
              {canTimezone && (
                <TimezoneInput
                  placeholder={t('common.form.timezoneSystem', {})}
                  description={time ?? undefined}
                  inputWrapperOrder={HELPER_UNDER}
                  allowDeselect
                  clearable
                  {...timezone.getInputProps('timezone')}
                />
              )}
            </div>
          )}

          {canAutokill && (
            <>
              <Switch
                label={t(`${base}.autokill.title`, {})}
                {...autokill.getInputProps('enabled', { type: 'checkbox' })}
              />
              <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
                <NumberInput
                  label={t(`${base}.autokill.form.secondsUntilAutoKill`, {})}
                  {...autokill.getInputProps('seconds')}
                />
              </div>
            </>
          )}

          {canTrash && (
            <>
              <Switch
                label={t('common.elements.trashSettings.custom', {})}
                description={
                  custom
                    ? undefined
                    : t('common.elements.trashSettings.default', {
                        summary: formatTrashSettings(server.trash.defaults),
                      })
                }
                checked={custom !== null}
                onChange={(e) => trash.setFieldValue('custom', e.currentTarget.checked ? server.trash.defaults : null)}
              />
              {custom && (
                <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
                  <TrashRetentionInput
                    label={t('common.elements.trashSettings.retention', {})}
                    description={t('common.elements.trashSettings.retentionDescription', {})}
                    inputWrapperOrder={HELPER_UNDER}
                    value={custom.retention}
                    onChange={(retention) => trash.setFieldValue('custom', { ...custom, retention })}
                    error={trash.errors['custom.retention']}
                  />
                  <TrashUsageThresholdInput
                    label={t('common.elements.trashSettings.usageThreshold', {})}
                    description={t('common.elements.trashSettings.usageThresholdDescription', {})}
                    inputWrapperOrder={HELPER_UNDER}
                    value={custom.usageThreshold}
                    onChange={(usageThreshold) => trash.setFieldValue('custom', { ...custom, usageThreshold })}
                    error={trash.errors['custom.usageThreshold']}
                  />
                </div>
              )}
            </>
          )}
        </div>
      </TitleCard>
    </form>
  );
}
