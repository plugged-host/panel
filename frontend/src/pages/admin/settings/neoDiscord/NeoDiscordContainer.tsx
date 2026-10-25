// plugged.host: Admin › Settings › Accounts › neo on Discord (docs/features/neo-discord.md). The
// assistant bot's switch and token are on the Discord tab; neo's limits, models and retention on neo's.
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { z } from 'zod';
import updateNeoDiscordSettings from '@/api/admin/settings/updateNeoDiscordSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsNeoDiscordSchema } from '@/lib/schemas/admin/settings.ts';
import { liveValidation } from '@/plugins/form/liveValidation.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type NeoDiscordValues = z.infer<typeof adminSettingsNeoDiscordSchema>;

const neoDiscordEmptyFormValues: NeoDiscordValues = {
  nameCalls: false,
  messagesPerMinute: 6,
  maxSteps: 6,
  historyMinutes: 60,
  historyTurns: 6,
};

export default function NeoDiscordContainer() {
  const { t } = useTranslations();
  const neoDiscord = useAdminStore((state) => state.neoDiscord);
  const page = 'pages.admin.settings.tabs.neoDiscord.page';

  const form = useForm<NeoDiscordValues>({
    initialValues: neoDiscordEmptyFormValues,
    ...liveValidation,
    validate: zod4Resolver(adminSettingsNeoDiscordSchema),
  });

  useHydrateForm(form, neoDiscord, (values: NeoDiscordValues) => values);
  useFormDraft(form, 'neoDiscord');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsNeoDiscordSchema,
    storeKey: 'neoDiscord',
    update: updateNeoDiscordSettings,
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<NeoDiscordValues>[] = [
    {
      type: 'switch',
      name: 'nameCalls',
      label: t(`${page}.form.nameCalls`, {}),
      description: t(`${page}.form.nameCallsDescription`, {}),
    },
    {
      type: 'number',
      name: 'messagesPerMinute',
      label: t(`${page}.form.messagesPerMinute`, {}),
      description: t(`${page}.form.messagesPerMinuteDescription`, {}),
      required: true,
      props: { min: 1, max: 60 },
    },
    {
      type: 'number',
      name: 'maxSteps',
      label: t(`${page}.form.maxSteps`, {}),
      description: t(`${page}.form.maxStepsDescription`, {}),
      required: true,
      props: { min: 2, max: 20 },
    },
    {
      type: 'number',
      name: 'historyMinutes',
      label: t(`${page}.form.historyMinutes`, {}),
      description: t(`${page}.form.historyMinutesDescription`, {}),
      required: true,
      props: { min: 1, max: 1440 },
    },
    {
      type: 'number',
      name: 'historyTurns',
      label: t(`${page}.form.historyTurns`, {}),
      description: t(`${page}.form.historyTurnsDescription`, {}),
      required: true,
      props: { min: 0, max: 20 },
    },
  ];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <Stack gap='md'>
        <Text size='sm' c='dimmed'>
          {t(`${page}.intro`, {})}
        </Text>

        <form onSubmit={form.onSubmit(submit)}>
          <FormEngine form={form} fields={fields} />

          <Group mt='md'>
            <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
          </Group>
        </form>
      </Stack>
    </AdminSubContentContainer>
  );
}
