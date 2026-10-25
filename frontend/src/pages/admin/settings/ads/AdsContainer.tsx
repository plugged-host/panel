// plugged.host: Admin › Settings › Ads, one tab per part (docs/features/ads.md)
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateAdsSettings from '@/api/admin/settings/updateAdsSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsAdsSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type AdsValues = z.infer<typeof adminSettingsAdsSchema>;

export type AdsPart = 'placement' | 'adsterra' | 'monetag' | 'room' | 'adblock' | 'inGame';

// What each tab edits. A tab saves the whole section: its own fields over what is stored, so an
// unsaved edit in another tab is neither sent nor lost.
const partFields: Record<AdsPart, (keyof AdsValues)[]> = {
  placement: [
    'enabled',
    'showAccount',
    'showServer',
    'memberFree',
    'slotTop',
    'slotServerTools',
    'slotPages',
    'slotFoot',
    'maxUnitsPerPage',
    'refreshSeconds',
    'refreshMemberSeconds',
  ],
  adsterra: [
    'zones',
    'frameOrigin',
    'nativeScript',
    'nativeContainer',
    'nativeHeight',
    'socialBarScript',
    'socialBarEverywhere',
    'popunderScript',
    'roomScript',
    'cspHosts',
  ],
  monetag: [
    'monetagPushZone',
    'monetagPushSrc',
    'monetagVignetteZone',
    'monetagVignetteSrc',
    'monetagVignetteMinutes',
    'monetagVignetteMembers',
    'monetagVerification',
  ],
  room: [
    'roomEnabled',
    'roomCpu',
    'roomMemory',
    'roomDisk',
    'roomMinSeconds',
    'roomMaxSeconds',
    'roomPassMinutes',
    'roomBanner',
    'roomNative',
    'roomSocialBar',
    'roomPopunder',
    'roomInterstitial',
  ],
  adblock: [
    'adblockEnabled',
    'adblockMode',
    'adblockGrace',
    'adblockDismissHours',
    'adblockProbe',
    'adblockSkipConsole',
    'adblockMessage',
  ],
  inGame: ['inGameEnabled', 'inGameMessage', 'inGameInterval'],
};

export default function AdsContainer({ part }: { part: AdsPart }) {
  const { t } = useTranslations();
  const ads = useAdminStore((state) => state.ads);
  const page = 'pages.admin.settings.tabs.ads.page';
  const fieldKey = `${page}.form`;

  const form = useForm<AdsValues>({
    initialValues: ads,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsAdsSchema),
  });

  useHydrateForm(form, ads, (values: AdsValues) => values);
  useFormDraft(form, `ads.${part}`);

  const schema = adminSettingsAdsSchema.transform(
    (values): AdsValues => ({
      ...ads,
      ...(Object.fromEntries(partFields[part].map((field) => [field, values[field]])) as Partial<AdsValues>),
    }),
  );

  const { loading, submit } = useSettingsSection({
    form,
    schema,
    storeKey: 'ads',
    update: updateAdsSettings,
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const label = (name: keyof AdsValues & string) => t(`${fieldKey}.${name}`, {});
  const description = (name: keyof AdsValues & string) => t(`${fieldKey}.${name}Description`, {});

  const toggle = (name: keyof AdsValues & string, when?: (values: AdsValues) => boolean): FieldDef<AdsValues> => ({
    type: 'switch',
    name,
    label: label(name),
    description: description(name),
    ...(when ? { when } : {}),
  });

  const number = (
    name: keyof AdsValues & string,
    min: number,
    max: number,
    when?: (values: AdsValues) => boolean,
  ): FieldDef<AdsValues> => ({
    type: 'number',
    name,
    label: label(name),
    description: description(name),
    required: true,
    props: { min, max },
    ...(when ? { when } : {}),
  });

  const text = (name: keyof AdsValues & string, when?: (values: AdsValues) => boolean): FieldDef<AdsValues> => ({
    type: 'text',
    name,
    label: label(name),
    description: description(name),
    ...(when ? { when } : {}),
  });

  const room = (values: AdsValues) => values.roomEnabled;
  const adblock = (values: AdsValues) => values.adblockEnabled;
  const inGame = (values: AdsValues) => values.inGameEnabled;

  const fields: Record<AdsPart, FieldDef<AdsValues>[]> = {
    placement: [
      toggle('enabled'),
      toggle('showAccount'),
      toggle('showServer'),
      toggle('memberFree'),
      { type: 'divider', name: 'slotsDivider', label: t(`${fieldKey}.slots`, {}) },
      toggle('slotTop'),
      toggle('slotServerTools'),
      toggle('slotPages'),
      toggle('slotFoot'),
      number('maxUnitsPerPage', 0, 20),
      { type: 'divider', name: 'refreshDivider', label: t(`${fieldKey}.refresh`, {}) },
      number('refreshSeconds', 0, 3600),
      number('refreshMemberSeconds', 0, 3600),
    ],
    adsterra: [
      {
        type: 'textarea',
        name: 'zones',
        label: label('zones'),
        description: description('zones'),
        rows: 6,
        colSpan: 'full',
      },
      text('frameOrigin'),
      { type: 'divider', name: 'nativeDivider', label: t(`${fieldKey}.native`, {}) },
      text('nativeScript'),
      text('nativeContainer'),
      number('nativeHeight', 50, 2000),
      { type: 'divider', name: 'pageDivider', label: t(`${fieldKey}.pageScripts`, {}) },
      text('socialBarScript'),
      toggle('socialBarEverywhere'),
      text('popunderScript'),
      text('roomScript'),
      { type: 'textarea', name: 'cspHosts', label: label('cspHosts'), description: description('cspHosts'), rows: 3 },
    ],
    monetag: [
      text('monetagPushZone'),
      text('monetagPushSrc'),
      text('monetagVignetteZone'),
      text('monetagVignetteSrc'),
      number('monetagVignetteMinutes', 0, 1440),
      toggle('monetagVignetteMembers'),
      text('monetagVerification'),
    ],
    room: [
      toggle('roomEnabled'),
      number('roomCpu', 0, 1_000_000, room),
      {
        type: 'size',
        name: 'roomMemory',
        label: label('roomMemory'),
        description: description('roomMemory'),
        mode: 'mb',
        min: 0,
        when: room,
      },
      {
        type: 'size',
        name: 'roomDisk',
        label: label('roomDisk'),
        description: description('roomDisk'),
        mode: 'mb',
        min: 0,
        when: room,
      },
      number('roomMinSeconds', 0, 600, room),
      number('roomMaxSeconds', 1, 600, room),
      number('roomPassMinutes', 0, 1440, room),
      { type: 'divider', name: 'carriesDivider', label: t(`${fieldKey}.carries`, {}) },
      toggle('roomBanner'),
      toggle('roomNative'),
      toggle('roomSocialBar'),
      toggle('roomPopunder'),
      toggle('roomInterstitial'),
    ],
    adblock: [
      toggle('adblockEnabled'),
      {
        type: 'select',
        name: 'adblockMode',
        label: label('adblockMode'),
        description: description('adblockMode'),
        required: true,
        options: [
          { value: 'notice', label: t(`${fieldKey}.adblockModes.notice`, {}) },
          { value: 'modal', label: t(`${fieldKey}.adblockModes.modal`, {}) },
          { value: 'block', label: t(`${fieldKey}.adblockModes.block`, {}) },
        ],
        when: adblock,
      },
      number('adblockGrace', 0, 60, (values) => values.adblockEnabled && values.adblockMode === 'modal'),
      number('adblockDismissHours', 0, 720, (values) => values.adblockEnabled && values.adblockMode !== 'block'),
      toggle('adblockProbe', adblock),
      toggle('adblockSkipConsole', adblock),
      {
        type: 'textarea',
        name: 'adblockMessage',
        label: label('adblockMessage'),
        description: description('adblockMessage'),
        rows: 3,
        when: adblock,
      },
    ],
    inGame: [toggle('inGameEnabled'), text('inGameMessage', inGame), number('inGameInterval', 5, 1440, inGame)],
  };

  return (
    <AdminSubContentContainer title={t(`${page}.titles.${part}`, {})} titleOrder={2}>
      <form onSubmit={form.onSubmit(submit)}>
        <FormEngine form={form} fields={fields[part]} />

        <Group mt='md'>
          <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
        </Group>
      </form>
    </AdminSubContentContainer>
  );
}
