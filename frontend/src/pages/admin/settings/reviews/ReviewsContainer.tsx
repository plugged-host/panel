// plugged.host: Admin › Settings › Accounts › Reviews (docs/features/reviews.md)
import { z } from 'zod';
import updateReviewsSettings from '@/api/admin/settings/updateReviewsSettings.ts';
import getSettings from '@/api/getSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsReviewsSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type ReviewsValues = z.infer<typeof adminSettingsReviewsSchema>;

const reviewsEmptyFormValues: ReviewsValues = {
  url: '',
  site: '',
  delayDays: 3,
  intervalDays: 180,
};

export default function ReviewsContainer() {
  const { t } = useTranslations();
  const reviews = useAdminStore((state) => state.reviews);
  const page = 'pages.admin.settings.tabs.reviews.page';

  const form = useFormEngine<ReviewsValues>('admin.settings.reviews', {
    schema: adminSettingsReviewsSchema,
    initialValues: reviewsEmptyFormValues,
  });

  useHydrateForm(form, reviews, (values: ReviewsValues) => values);
  useFormDraft(form, 'reviews');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsReviewsSchema,
    storeKey: 'reviews',
    // the account menu's link follows at once, with the site name the panel works out
    update: (values) =>
      updateReviewsSettings(values).then(() =>
        getSettings()
          .then((settings) => useGlobalStore.getState().updateSettings({ reviews: settings.reviews }))
          .catch(() => undefined),
      ),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<ReviewsValues>[] = [
    {
      type: 'text',
      name: 'url',
      label: t(`${page}.form.url`, {}),
      description: t(`${page}.form.urlDescription`, {}),
      props: { placeholder: 'https://reviews.example.com/evaluate/panel.example.com' },
    },
    {
      type: 'text',
      name: 'site',
      label: t(`${page}.form.site`, {}),
      description: t(`${page}.form.siteDescription`, {}),
    },
    {
      type: 'number',
      name: 'delayDays',
      label: t(`${page}.form.delayDays`, {}),
      description: t(`${page}.form.delayDaysDescription`, {}),
      required: true,
      props: { min: 0, max: 365 },
    },
    {
      type: 'number',
      name: 'intervalDays',
      label: t(`${page}.form.intervalDays`, {}),
      description: t(`${page}.form.intervalDaysDescription`, {}),
      required: true,
      props: { min: 0, max: 3650 },
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
