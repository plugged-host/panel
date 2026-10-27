// plugged.host: Settings' "Notifications": the person's backup mails for this server, two switches saved as
// soon as they flip (shared/src/backup_notifications.rs, docs/features/server-settings.md#notifications)
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import updateServerNotifications from '@/api/server/settings/updateServerNotifications.ts';
import Switch from '@/elements/input/Switch.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { serverSettingsNotificationsSchema } from '@/lib/schemas/server/settings.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import SettingsSection from './SettingsSection.tsx';
import { useNotificationSettings } from './useSettingsSections.ts';

type Notifications = z.infer<typeof serverSettingsNotificationsSchema>;

const base = 'pages.server.settings.notifications';

export default function NotificationsSection() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const resource = useNotificationSettings();
  const [saving, setSaving] = useState(false);
  // what was last saved here, shown right away instead of waiting for a refetch
  const [saved, setSaved] = useState<Notifications | null>(null);

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
    <SettingsSection id='notifications' title={t(`${base}.title`, {})}>
      <div className='theme-eco-card'>
        <ResourceView resource={resource}>
          {(stored) => {
            const notifications = saved ?? stored;

            return (
              <>
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
              </>
            );
          }}
        </ResourceView>
      </div>
    </SettingsSection>
  );
}
