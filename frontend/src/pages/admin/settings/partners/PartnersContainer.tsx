// plugged.host: Admin › Settings › Economy › Partners (docs/features/partners.md)
import { z } from 'zod';
import updatePartnersSettings from '@/api/admin/settings/updatePartnersSettings.ts';
import getSettings from '@/api/getSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsPartnersSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type PartnersValues = z.infer<typeof adminSettingsPartnersSchema>;

const partnersEmptyFormValues: PartnersValues = {
  enabled: false,
  minMembers: 500,
  minCredits: 500,
  maxCredits: 5000,
  cooldownDays: 30,
  graceDays: 7,
  channelRequired: true,
  channelNames: '',
  channelLinks: '',
  neoAssessment: true,
  guidance: '',
};

export default function PartnersContainer() {
  const { t } = useTranslations();
  const partners = useAdminStore((state) => state.partners);
  const appName = useGlobalStore((state) => state.settings.app.name);
  const page = 'pages.admin.settings.tabs.partners.page';

  const form = useFormEngine<PartnersValues>('admin.settings.partners', {
    schema: adminSettingsPartnersSchema,
    initialValues: partnersEmptyFormValues,
  });

  useHydrateForm(form, partners, (values: PartnersValues) => values);
  useFormDraft(form, 'partners');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsPartnersSchema,
    storeKey: 'partners',
    // the account menu's row follows at once
    update: (values) =>
      updatePartnersSettings(values).then(() =>
        getSettings()
          .then((settings) => useGlobalStore.getState().updateSettings({ partners: settings.partners }))
          .catch(() => undefined),
      ),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<PartnersValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.form.enabled`, {}),
      description: t(`${page}.form.enabledDescription`, {}),
    },
    {
      type: 'number',
      name: 'minMembers',
      label: t(`${page}.form.minMembers`, {}),
      description: t(`${page}.form.minMembersDescription`, {}),
      required: true,
      props: { min: 1, max: 10_000_000 },
    },
    {
      type: 'number',
      name: 'minCredits',
      label: t(`${page}.form.minCredits`, {}),
      description: t(`${page}.form.minCreditsDescription`, {}),
      required: true,
      props: { min: 0, max: 10_000_000 },
    },
    {
      type: 'number',
      name: 'maxCredits',
      label: t(`${page}.form.maxCredits`, {}),
      description: t(`${page}.form.maxCreditsDescription`, {}),
      required: true,
      props: { min: 0, max: 10_000_000 },
    },
    {
      type: 'number',
      name: 'graceDays',
      label: t(`${page}.form.graceDays`, {}),
      description: t(`${page}.form.graceDaysDescription`, {}),
      required: true,
      props: { min: 0, max: 60 },
    },
    {
      type: 'number',
      name: 'cooldownDays',
      label: t(`${page}.form.cooldownDays`, {}),
      description: t(`${page}.form.cooldownDaysDescription`, {}),
      required: true,
      props: { min: 0, max: 365 },
    },
    {
      type: 'switch',
      name: 'channelRequired',
      label: t(`${page}.form.channelRequired`, {}),
      description: t(`${page}.form.channelRequiredDescription`, { app: appName }),
    },
    {
      type: 'textarea',
      name: 'channelNames',
      label: t(`${page}.form.channelNames`, {}),
      description: t(`${page}.form.channelNamesDescription`, { app: appName }),
      props: { autosize: true, minRows: 2, maxRows: 6, placeholder: 'partners' },
    },
    {
      type: 'textarea',
      name: 'channelLinks',
      label: t(`${page}.form.channelLinks`, {}),
      description: t(`${page}.form.channelLinksDescription`, {}),
      props: { autosize: true, minRows: 2, maxRows: 6, placeholder: 'example.com' },
    },
    {
      type: 'switch',
      name: 'neoAssessment',
      label: t(`${page}.form.neoAssessment`, {}),
      description: t(`${page}.form.neoAssessmentDescription`, {}),
    },
    {
      type: 'textarea',
      name: 'guidance',
      label: t(`${page}.form.guidance`, {}),
      description: t(`${page}.form.guidanceDescription`, {}),
      props: { autosize: true, minRows: 3, maxRows: 10 },
    },
  ];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
