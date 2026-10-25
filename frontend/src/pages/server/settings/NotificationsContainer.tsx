// plugged.host: the Pelican panel's "Notifications" section (ServerSettings::notificationsSection):
// the person's backup mails for this server, saved as soon as a switch flips
// (shared/src/backup_notifications.rs, docs/features/server-settings.md)
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerNotifications from '@/api/server/settings/getServerNotifications.ts';
import updateServerNotifications from '@/api/server/settings/updateServerNotifications.ts';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSettingsNotificationsSchema } from '@/lib/schemas/server/settings.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Notifications = z.infer<typeof serverSettingsNotificationsSchema>;

const base = 'pages.server.settings.notifications';

export default function NotificationsContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [saving, setSaving] = useState(false);
  // what was last saved here, shown right away instead of waiting for a refetch
  const [saved, setSaved] = useState<Notifications | null>(null);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.notifications(),
    queryFn: () => getServerNotifications(server.uuid),
  });

  const save = (next: Notifications) => {
    setSaving(true);
    setSaved(next);
    updateServerNotifications(server.uuid, next)
      .then((stored) => {
        setSaved(stored);
        addToast(t(`${base}.saved`, {}), 'success');
      })
      .catch((err) => {
        setSaved(null);
        addToast(httpErrorToHuman(err), 'error');
      })
      .finally(() => {
        setSaving(false);
        resource.invalidate();
      });
  };

  return (
    <TitleCard title={t(`${base}.title`, {})}>
      <ResourceView resource={resource}>
        {(stored) => {
          const notifications = saved ?? stored;

          return (
            <Stack>
              <Switch
                label={t(`${base}.manual`, {})}
                description={t(`${base}.manualHint`, {})}
                checked={notifications.manualBackups}
                disabled={saving}
                onChange={(event) => save({ ...notifications, manualBackups: event.currentTarget.checked })}
              />
              <Switch
                label={t(`${base}.scheduled`, {})}
                description={t(`${base}.scheduledHint`, {})}
                checked={notifications.scheduledBackups}
                disabled={saving}
                onChange={(event) => save({ ...notifications, scheduledBackups: event.currentTarget.checked })}
              />
            </Stack>
          );
        }}
      </ResourceView>
    </TitleCard>
  );
}
