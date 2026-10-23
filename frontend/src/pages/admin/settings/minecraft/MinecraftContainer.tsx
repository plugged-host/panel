// plugged.host: Admin › Settings › Hosting › Minecraft (docs/features/minecraft.md)
import { useState } from 'react';
import { z } from 'zod';
import updateMinecraftSettings from '@/api/admin/settings/updateMinecraftSettings.ts';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Group from '@/elements/layout/Group.tsx';
import { adminSettingsMinecraftSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type MinecraftValues = z.infer<typeof adminSettingsMinecraftSchema>;

const minecraftEmptyFormValues: MinecraftValues = {
  minecraftCurseforgeKey: '',
  minecraftSpigot: true,
  minecraftUserAgent: '',
};

export default function MinecraftContainer() {
  const { t } = useTranslations();
  const games = useAdminStore((state) => state.games);
  // a stored key comes back as a placeholder
  const [keyStored, setKeyStored] = useState(() => !!games.minecraftCurseforgeKey);
  const page = 'pages.admin.settings.tabs.minecraft.page';

  const form = useFormEngine<MinecraftValues>('admin.settings.minecraft', {
    schema: adminSettingsMinecraftSchema,
    initialValues: minecraftEmptyFormValues,
  });

  // the key never comes back: the field stays empty and keeps it
  useHydrateForm(form, games, (games) => ({
    minecraftCurseforgeKey: '',
    minecraftSpigot: games.minecraftSpigot,
    minecraftUserAgent: games.minecraftUserAgent,
  }));
  useFormDraft(form, 'minecraft');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsMinecraftSchema,
    storeKey: 'games',
    // a typed key is stored from now on
    update: (values) =>
      updateMinecraftSettings(values).then(() =>
        setKeyStored((stored) => stored || !!values.minecraftCurseforgeKey.trim()),
      ),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const fields: FieldDef<MinecraftValues>[] = [
    {
      type: 'password',
      name: 'minecraftCurseforgeKey',
      label: t(`${page}.form.curseforgeKey`, {}),
      description: keyStored ? t(`${page}.form.curseforgeKeyStored`, {}) : t(`${page}.form.curseforgeKeyMissing`, {}),
      props: { autoComplete: 'new-password' },
    },
    {
      type: 'switch',
      name: 'minecraftSpigot',
      label: t(`${page}.form.spigot`, {}),
      description: t(`${page}.form.spigotDescription`, {}),
    },
    {
      type: 'text',
      name: 'minecraftUserAgent',
      label: t(`${page}.form.userAgent`, {}),
      description: t(`${page}.form.userAgentDescription`, {}),
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
