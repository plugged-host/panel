// plugged.host: Admin › Settings › Hosting › Devboxes (docs/features/devboxes.md)
import { useState } from 'react';
import { z } from 'zod';
import createDevboxCertificates from '@/api/admin/settings/createDevboxCertificates.ts';
import getSettings from '@/api/admin/settings/getSettings.ts';
import updateDevboxesSettings from '@/api/admin/settings/updateDevboxesSettings.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import AdminSubContentContainer from '@/elements/containers/AdminSubContentContainer.tsx';
import { type FieldDef, FormEngine, useFormEngine } from '@/elements/form-engine/index.ts';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminSettingsDevboxesSchema } from '@/lib/schemas/admin/settings.ts';
import { useFormDraft } from '@/plugins/form/useFormDraft.ts';
import { useHydrateForm } from '@/plugins/form/useHydrateForm.ts';
import { useGroupedEggOptions } from '@/plugins/useGroupedEggOptions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useAdminStore } from '@/stores/admin.tsx';
import SettingsSaveButton from '../SettingsSaveButton.tsx';
import { useSettingsSection } from '../useSettingsSection.ts';

type DevboxesValues = z.infer<typeof adminSettingsDevboxesSchema>;
type NumberField =
  | 'cpu'
  | 'memory'
  | 'disk'
  | 'premiumCpu'
  | 'premiumMemory'
  | 'premiumDisk'
  | 'idleMinutes'
  | 'maxHours'
  | 'runningPerUser'
  | 'cleanupDays';

const devboxesEmptyFormValues: DevboxesValues = {
  enabled: false,
  eggUuid: null,
  image: '',
  membersOnly: true,
  cpu: 200,
  memory: 2048,
  disk: 5120,
  premiumCpu: 400,
  premiumMemory: 4096,
  premiumDisk: 10240,
  idleMinutes: 15,
  maxHours: 12,
  runningPerUser: 2,
  cleanupDays: 14,
  panelUrl: '',
  webDomain: '',
  webOriginIp: '',
  cloudflareToken: '',
};

const page = 'pages.admin.settings.tabs.devboxes.page';

function Certificates() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const devboxes = useAdminStore((state) => state.devboxes);
  const updateSettings = useAdminStore((state) => state.updateSettings);
  const [creating, setCreating] = useState(false);
  const stored = !!devboxes.caCert && !!devboxes.edgeCert;

  const create = () => {
    setCreating(true);
    createDevboxCertificates()
      .then(() => getSettings())
      .then((settings) => {
        updateSettings({ devboxes: settings.devboxes });
        addToast(t(`${page}.toast.certificates`, {}), 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setCreating(false));
  };

  return (
    <Stack gap='xs'>
      <Text size='sm' c='dimmed'>
        {stored ? t(`${page}.certificatesStored`, {}) : t(`${page}.certificatesMissing`, {})}
      </Text>
      <Group>
        <Button variant='default' loading={creating} onClick={create}>
          {stored ? t(`${page}.button.replaceCertificates`, {}) : t(`${page}.button.createCertificates`, {})}
        </Button>
      </Group>
    </Stack>
  );
}

export default function DevboxesContainer() {
  const { t } = useTranslations();
  const devboxes = useAdminStore((state) => state.devboxes);
  const { eggOptions, loading: eggsLoading } = useGroupedEggOptions();
  // a stored token comes back as a placeholder
  const [tokenStored, setTokenStored] = useState(() => !!devboxes.cloudflareToken);

  const form = useFormEngine<DevboxesValues>('admin.settings.devboxes', {
    schema: adminSettingsDevboxesSchema,
    initialValues: devboxesEmptyFormValues,
  });

  // the token never comes back: the field stays empty and keeps it
  useHydrateForm(form, devboxes, ({ caCert: _caCert, edgeCert: _edgeCert, ...devboxes }) => ({
    ...devboxes,
    cloudflareToken: '',
  }));
  useFormDraft(form, 'devboxes');

  const { loading, submit } = useSettingsSection({
    form,
    schema: adminSettingsDevboxesSchema,
    storeKey: 'devboxes',
    // a typed token is stored from now on
    update: (values) =>
      updateDevboxesSettings(values).then(() => setTokenStored((stored) => stored || !!values.cloudflareToken.trim())),
    successMessage: t(`${page}.toast.updated`, {}),
  });

  const number = (name: NumberField, min: number, max: number, description?: string): FieldDef<DevboxesValues> => ({
    type: 'number',
    name,
    label: t(`${page}.form.${name}`, {}),
    description,
    required: true,
    props: { min, max },
  });

  const fields: FieldDef<DevboxesValues>[] = [
    {
      type: 'switch',
      name: 'enabled',
      label: t(`${page}.form.enabled`, {}),
      description: t(`${page}.form.enabledDescription`, {}),
    },
    {
      type: 'custom',
      name: 'eggUuid',
      render: (f) => (
        <Select
          label={t(`${page}.form.egg`, {})}
          description={t(`${page}.form.eggDescription`, {})}
          placeholder={t(`${page}.form.eggPlaceholder`, {})}
          data={eggOptions}
          searchable
          clearable
          disabled={eggsLoading}
          key={f.key('eggUuid')}
          {...f.getInputProps('eggUuid')}
        />
      ),
    },
    {
      type: 'text',
      name: 'image',
      label: t(`${page}.form.image`, {}),
      description: t(`${page}.form.imageDescription`, {}),
    },
    {
      type: 'switch',
      name: 'membersOnly',
      label: t(`${page}.form.membersOnly`, {}),
      description: t(`${page}.form.membersOnlyDescription`, {}),
    },
    {
      type: 'divider',
      name: 'resourcesDivider',
      label: t(`${page}.form.resources`, {}),
    },
    number('cpu', 25, 3200, t(`${page}.form.resourcesDescription`, {})),
    number('memory', 256, 65536),
    number('disk', 512, 262144),
    number('premiumCpu', 25, 3200, t(`${page}.form.premiumDescription`, {})),
    number('premiumMemory', 256, 65536),
    number('premiumDisk', 512, 262144),
    {
      type: 'divider',
      name: 'lifecycleDivider',
      label: t(`${page}.form.lifecycle`, {}),
    },
    number('idleMinutes', 1, 1440, t(`${page}.form.idleMinutesDescription`, {})),
    number('maxHours', 1, 168, t(`${page}.form.maxHoursDescription`, {})),
    number('runningPerUser', 1, 20, t(`${page}.form.runningPerUserDescription`, {})),
    number('cleanupDays', 1, 365, t(`${page}.form.cleanupDaysDescription`, {})),
    {
      type: 'text',
      name: 'panelUrl',
      label: t(`${page}.form.panelUrl`, {}),
      description: t(`${page}.form.panelUrlDescription`, {}),
    },
    {
      type: 'divider',
      name: 'browserDivider',
      label: t(`${page}.form.browser`, {}),
    },
    {
      type: 'text',
      name: 'webDomain',
      label: t(`${page}.form.webDomain`, {}),
      description: t(`${page}.form.webDomainDescription`, {}),
    },
    {
      type: 'text',
      name: 'webOriginIp',
      label: t(`${page}.form.webOriginIp`, {}),
      description: t(`${page}.form.webOriginIpDescription`, {}),
    },
    {
      type: 'password',
      name: 'cloudflareToken',
      label: t(`${page}.form.cloudflareToken`, {}),
      description: tokenStored
        ? t(`${page}.form.cloudflareTokenStored`, {})
        : t(`${page}.form.cloudflareTokenMissing`, {}),
      props: { autoComplete: 'new-password' },
    },
  ];

  return (
    <AdminSubContentContainer title={t(`${page}.title`, {})} titleOrder={2}>
      <Stack gap='xl'>
        <form onSubmit={form.onSubmit(submit)}>
          <FormEngine form={form} fields={fields} />

          <Group mt='md'>
            <SettingsSaveButton loading={loading} disabled={!form.isValid()} />
          </Group>
        </form>

        <Stack gap='xs'>
          <Text fw={600}>{t(`${page}.certificates`, {})}</Text>
          <Certificates />
        </Stack>
      </Stack>
    </AdminSubContentContainer>
  );
}
