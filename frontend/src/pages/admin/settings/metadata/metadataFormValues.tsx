import { ColorInput } from '@mantine/core';
import { z } from 'zod';
import type { FieldDef } from '@/elements/form-engine/index.ts';
import { ACCENT_PRESETS, accentSwatch, DEFAULT_ACCENT } from '@/lib/accent.ts';
import { adminSettingsMetadataSchema } from '@/lib/schemas/admin/settings.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type MetadataFormValues = z.infer<typeof adminSettingsMetadataSchema>;

export const metadataEmptyFormValues: MetadataFormValues = {
  description: null,
  themeColor: DEFAULT_ACCENT,
  ogImage: null,
  twitterCard: 'summary_large_image',
  indexable: true,
};

export const metadataToFormValues = (metadata: MetadataFormValues): Partial<MetadataFormValues> => ({ ...metadata });

interface MetadataFormFieldsOptions {
  assetUrls: string[];
  descriptionPlaceholder: string;
  ogImagePlaceholder: string;
}

export function useMetadataFormFields({
  assetUrls,
  descriptionPlaceholder,
  ogImagePlaceholder,
}: MetadataFormFieldsOptions): FieldDef<MetadataFormValues>[] {
  const { t } = useTranslations();

  return [
    {
      type: 'textarea',
      name: 'description',
      label: t('pages.admin.settings.tabs.metadata.page.form.description', {}),
      description: t('pages.admin.settings.tabs.metadata.page.form.descriptionDescription', {}),
      rows: 3,
      colSpan: 'full',
      props: { placeholder: descriptionPlaceholder },
    },
    {
      type: 'autocomplete',
      name: 'ogImage',
      label: t('pages.admin.settings.tabs.metadata.page.form.ogImage', {}),
      description: t('pages.admin.settings.tabs.metadata.page.form.ogImageDescription', {}),
      options: assetUrls,
      props: { placeholder: ogImagePlaceholder },
    },
    {
      type: 'select',
      name: 'twitterCard',
      label: t('pages.admin.settings.tabs.metadata.page.form.twitterCard', {}),
      required: true,
      options: [
        { label: t('pages.admin.settings.tabs.metadata.page.enum.twitterCard.summary', {}), value: 'summary' },
        {
          label: t('pages.admin.settings.tabs.metadata.page.enum.twitterCard.summaryLargeImage', {}),
          value: 'summary_large_image',
        },
      ],
    },
    // plugged.host: the theme colour is Theme v2's accent; the swatches are its presets (Tailwind's
    // palettes), like the Pelican panel's Theme v2 settings.
    {
      type: 'custom',
      name: 'themeColor',
      render: (form) => (
        <ColorInput
          label={t('pages.admin.settings.tabs.metadata.page.form.themeColor', {})}
          description={t('pages.admin.settings.tabs.metadata.page.form.themeColorDescription', {})}
          withAsterisk
          format='hex'
          swatches={ACCENT_PRESETS.map(accentSwatch)}
          swatchesPerRow={9}
          {...form.getInputProps('themeColor')}
        />
      ),
    },
    {
      type: 'switch',
      name: 'indexable',
      label: t('pages.admin.settings.tabs.metadata.page.form.indexable', {}),
    },
  ];
}
