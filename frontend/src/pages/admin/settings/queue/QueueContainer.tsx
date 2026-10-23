// plugged.host: Admin › Settings › Hosting › Start Queue (docs/features/start-queue.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateQueueSettings from '@/api/admin/settings/updateQueueSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsQueueSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type QueueValues = z.infer<typeof adminSettingsQueueSchema>;

const queueEmptyFormValues: QueueValues = {
  enabled: false,
  includeRestart: true,
  linePercent: 85,
  maxWait: 600,
  releaseInterval: 20,
  passMinutes: 10,
  moveOffer: true,
  moveRequiresMembership: true,
};

export default function QueueContainer() {
  const { t } = useTranslations();
  const queue = useAdminStore((state) => state.queue);
  const page = 'pages.admin.settings.tabs.queue.page.form';

  const form = useForm<QueueValues>({
    initialValues: queueEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsQueueSchema),
  });

  useHydrateForm(form, queue, (values: QueueValues) => values);
  useFormDraft(form, 'queue');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsQueueSchema,
    storeKey: 'queue',
    update: updateQueueSettings,
    successMessage: t('pages.admin.settings.tabs.queue.page.toast.updated', {}),
  });

  const fields: FieldDef<QueueValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.enabled`, {}),
      description: t(`${page}.enabledDescription`, {}),
    },
    {
      type: 'switch',
      name: 'includeRestart',
      label: t(`${page}.includeRestart`, {}),
      description: t(`${page}.includeRestartDescription`, {}),
    },
    {
      type: 'number',
      name: 'linePercent',
      label: t(`${page}.linePercent`, {}),
      description: t(`${page}.linePercentDescription`, {}),
      required: true,
      props: { min: 1, max: 100 },
    },
    {
      type: 'number',
      name: 'maxWait',
      label: t(`${page}.maxWait`, {}),
      description: t(`${page}.maxWaitDescription`, {}),
      required: true,
      props: { min: 30, max: 86400 },
    },
    {
      type: 'number',
      name: 'releaseInterval',
      label: t(`${page}.releaseInterval`, {}),
      description: t(`${page}.releaseIntervalDescription`, {}),
      required: true,
      props: { min: 0, max: 3600 },
    },
    {
      type: 'number',
      name: 'passMinutes',
      label: t(`${page}.passMinutes`, {}),
      description: t(`${page}.passMinutesDescription`, {}),
      required: true,
      props: { min: 0, max: 1440 },
    },
    {
      type: 'divider',
      name: 'moveDivider',
      label: t(`${page}.move`, {}),
    },
    {
      type: 'switch',
      name: 'moveOffer',
      label: t(`${page}.moveOffer`, {}),
      description: t(`${page}.moveOfferDescription`, {}),
    },
    {
      type: 'switch',
      name: 'moveRequiresMembership',
      label: t(`${page}.moveRequiresMembership`, {}),
      description: t(`${page}.moveRequiresMembershipDescription`, {}),
    },
  ];

  return (
    <AdminSubContentContainer title={t('pages.admin.settings.tabs.queue.page.title', {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
