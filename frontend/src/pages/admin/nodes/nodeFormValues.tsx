import { faGlobe } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { z } from 'zod';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import { type FieldDef } from '@/elements/form-engine/index.ts';
import TextInput from '@/elements/input/TextInput.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import UrlMissingPortAlert from '@/elements/UrlMissingPortAlert.tsx';
import { WINGS_DEFAULT_PORT } from '@/lib/domain/node.ts';
import { getUrlConnectPort, withUrlPort } from '@/lib/network/url.ts';
import { adminBackupConfigurationSchema } from '@/lib/schemas/admin/backupConfigurations.ts';
import { adminLocationSchema } from '@/lib/schemas/admin/locations.ts';
import { adminNodeSchema, adminNodeUpdateSchema } from '@/lib/schemas/admin/nodes.ts';
import { useSearchableResource } from '@/plugins/resource/useSearchableResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type NodeFormValues = z.infer<typeof adminNodeUpdateSchema>;

export const nodeEmptyFormValues: NodeFormValues = {
  locationUuid: '',
  backupConfigurationUuid: null,
  name: '',
  deploymentEnabled: true,
  maintenanceEnabled: false,
  description: null,
  publicUrl: null,
  url: '',
  sftpHost: null,
  sftpPort: 2022,
  memory: 8192,
  disk: 10240,
  role: 'game', // plugged.host
  tags: [], // plugged.host
  srvTarget: null, // plugged.host
  cpuModel: null, // plugged.host
  memoryType: null, // plugged.host
  deploymentPaused: false, // plugged.host
};

export const nodeToFormValues = (node: z.infer<typeof adminNodeSchema>): Partial<NodeFormValues> => ({
  locationUuid: node.location.uuid,
  backupConfigurationUuid: node.backupConfiguration?.uuid ?? null,
  name: node.name,
  deploymentEnabled: node.deploymentEnabled,
  maintenanceEnabled: node.maintenanceEnabled,
  description: node.description,
  publicUrl: node.publicUrl,
  url: node.url,
  sftpHost: node.sftpHost,
  sftpPort: node.sftpPort,
  memory: node.memory,
  disk: node.disk,
  role: node.role, // plugged.host
  tags: node.tags, // plugged.host
  srvTarget: node.srvTarget, // plugged.host
  cpuModel: node.cpuModel, // plugged.host
  memoryType: node.memoryType, // plugged.host
  deploymentPaused: node.deploymentPaused, // plugged.host
});

interface NodeFormFieldsOptions {
  locations: ReturnType<typeof useSearchableResource<z.infer<typeof adminLocationSchema>>>;
  backupConfigurations: ReturnType<typeof useSearchableResource<z.infer<typeof adminBackupConfigurationSchema>>>;
  urlValue: string;
  isAIO: boolean;
  contextNode?: z.infer<typeof adminNodeSchema>;
}

export function useNodeFormFields({
  locations,
  backupConfigurations,
  urlValue,
  isAIO,
  contextNode,
}: NodeFormFieldsOptions): FieldDef<NodeFormValues>[] {
  const { t } = useTranslations();

  return [
    { type: 'text', name: 'name', label: t('common.form.name', {}), required: true },
    {
      type: 'select',
      name: 'locationUuid',
      label: t('common.table.columns.location', {}),
      required: true,
      options: locations.items.map((l) => ({ label: l.name, value: l.uuid })),
      props: {
        searchable: true,
        searchValue: locations.search,
        onSearchChange: locations.setSearch,
        loading: locations.loading,
      },
    },
    { type: 'textarea', name: 'description', label: t('common.form.description', {}), rows: 3, colSpan: 'full' },
    {
      type: 'divider',
      name: 'connectionDivider',
      label: t('pages.admin.nodes.tabs.general.page.section.connection', {}),
    },
    {
      type: 'custom',
      name: 'url',
      render: (f) => (
        <div className='flex flex-col gap-2'>
          <TextInput
            withAsterisk
            label={t('common.form.url', {})}
            description={t('pages.admin.nodes.tabs.general.page.form.urlDescription', {})}
            placeholder='https://node.example.com:8080'
            key={f.key('url')}
            {...f.getInputProps('url')}
            disabled={isAIO}
          />
          {!isAIO && (
            <UrlMissingPortAlert
              url={urlValue}
              defaultPort={WINGS_DEFAULT_PORT}
              onAddPort={() => f.setFieldValue('url', withUrlPort(urlValue, WINGS_DEFAULT_PORT))}
            >
              {t('pages.admin.nodes.tabs.general.page.alert.urlMissingPort', {
                port: String(getUrlConnectPort(urlValue) ?? 443),
                wingsPort: String(WINGS_DEFAULT_PORT),
              }).md()}
            </UrlMissingPortAlert>
          )}
        </div>
      ),
    },
    {
      type: 'custom',
      name: 'publicUrl',
      render: (f) => (
        <TextInput
          label={t('common.form.publicUrl', {})}
          description={t('pages.admin.nodes.tabs.general.page.form.publicUrlDescription', {})}
          placeholder='https://node.example.com:8080'
          key={f.key('publicUrl')}
          rightSection={
            <Tooltip label={t('pages.admin.nodes.tabs.general.page.tooltip.useWingsProxyUrl', {})}>
              <ActionIcon
                variant='subtle'
                onClick={() =>
                  f.setFieldValue('publicUrl', `${window.location.origin}/wings-proxy/${contextNode?.uuid}`)
                }
                disabled={!contextNode}
                size='lg'
              >
                <FontAwesomeIcon icon={faGlobe} />
              </ActionIcon>
            </Tooltip>
          }
          {...f.getInputProps('publicUrl')}
          disabled={isAIO}
        />
      ),
    },
    { type: 'text', name: 'sftpHost', label: t('common.form.sftpHost', {}) },
    // plugged.host
    {
      type: 'text',
      name: 'srvTarget',
      label: t('pages.admin.nodes.tabs.general.page.form.srvTarget', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.srvTargetDescription', {}),
      props: { placeholder: 'node.example.com' },
    },
    {
      type: 'number',
      name: 'sftpPort',
      label: t('common.form.sftpPort', {}),
      required: true,
      props: { min: 1, max: 65535 },
    },
    {
      type: 'divider',
      name: 'resourcesDivider',
      label: t('common.stat.resources', {}),
    },
    {
      type: 'size',
      name: 'memory',
      label: t('common.form.memory', {}),
      required: true,
      description: t('pages.admin.nodes.tabs.general.page.form.memoryDescription', {}),
      tooltip: t('pages.admin.nodes.tabs.general.page.form.unlimitedTooltip', {}),
      mode: 'mb',
      min: 0,
    },
    {
      type: 'size',
      name: 'disk',
      label: t('common.form.disk', {}),
      required: true,
      description: t('pages.admin.nodes.tabs.general.page.form.diskDescription', {}),
      tooltip: t('pages.admin.nodes.tabs.general.page.form.unlimitedTooltip', {}),
      mode: 'mb',
      min: 0,
    },
    // plugged.host: named on the public status page
    {
      type: 'text',
      name: 'cpuModel',
      label: t('pages.admin.nodes.tabs.general.page.form.cpuModel', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.cpuModelDescription', {}),
      props: { placeholder: 'Xeon E5-2651 v3', maxLength: 64 },
    },
    {
      type: 'text',
      name: 'memoryType',
      label: t('pages.admin.nodes.tabs.general.page.form.memoryType', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.memoryTypeDescription', {}),
      props: { placeholder: 'DDR4', maxLength: 32 },
    },
    {
      type: 'select',
      name: 'backupConfigurationUuid',
      label: t('common.form.backupConfiguration', {}),
      options: backupConfigurations.items.map((b) => ({ label: b.name, value: b.uuid })),
      props: {
        placeholder: t('pages.admin.nodes.tabs.general.page.form.backupConfigurationPlaceholder', {}),
        searchable: true,
        searchValue: backupConfigurations.search,
        onSearchChange: backupConfigurations.setSearch,
        allowDeselect: true,
        clearable: true,
        loading: backupConfigurations.loading,
      },
    },
    {
      type: 'divider',
      name: 'optionsDivider',
      label: t('pages.admin.nodes.tabs.general.page.section.options', {}),
    },
    { type: 'switch', name: 'deploymentEnabled', label: t('common.form.deploymentEnabled', {}) },
    { type: 'switch', name: 'maintenanceEnabled', label: t('common.form.maintenanceEnabled', {}) },
    // plugged.host
    {
      type: 'switch',
      name: 'deploymentPaused',
      label: t('pages.admin.nodes.tabs.general.page.form.deploymentPaused', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.deploymentPausedDescription', {}),
    },
    {
      type: 'select',
      name: 'role',
      label: t('pages.admin.nodes.tabs.general.page.form.role', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.roleDescription', {}),
      required: true,
      options: [
        { label: t('pages.admin.nodes.tabs.general.page.form.roleGame', {}), value: 'game' },
        { label: t('pages.admin.nodes.tabs.general.page.form.roleInternal', {}), value: 'internal' },
        { label: t('pages.admin.nodes.tabs.general.page.form.roleLimbo', {}), value: 'limbo' },
      ],
    },
    {
      type: 'tags',
      name: 'tags',
      label: t('pages.admin.nodes.tabs.general.page.form.tags', {}),
      description: t('pages.admin.nodes.tabs.general.page.form.tagsDescription', {}),
    },
  ];
}
