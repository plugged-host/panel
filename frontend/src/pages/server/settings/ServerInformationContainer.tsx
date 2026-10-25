// plugged.host: the Pelican panel's "Server Information" section (ServerSettings::generalSection,
// settings/server-meta.blade.php): name and description saved when they lose focus, the
// server's UUID, identifier and node, and its limits when the person cannot change its resources
// (docs/features/server-settings.md)
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerUsage from '@/api/server/settings/getServerUsage.ts';
import renameServer from '@/api/server/settings/renameServer.ts';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { CopyActionIcon, SettingRow, SettingRows } from '@/elements/SettingRows.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSettingsRenameSchema } from '@/lib/schemas/server/settings.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useResizeOptions } from './useSettingsSections.ts';

const base = 'pages.server.settings.general';

function Limits() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const usage = useResource({
    queryKey: queryKeys.server(server.uuid).settings.usage(),
    queryFn: () => getServerUsage(server.uuid),
    silent: true,
  }).data;

  const size = (megabytes: number) => (megabytes ? bytesToString(mbToBytes(megabytes)) : t(`${base}.unlimited`, {}));
  // "used / limit" as Pelican shows it, 0 meaning no limit; the limit alone when the list is hidden
  const count = (used: number | null | undefined, limit: number) => {
    const max = limit ? String(limit) : '∞';
    return used === null || used === undefined ? max : `${used} / ${max}`;
  };

  const limits = [
    {
      label: t(`${base}.limit.cpu`, {}),
      value: server.limits.cpu ? `${server.limits.cpu}%` : t(`${base}.unlimited`, {}),
    },
    { label: t(`${base}.limit.memory`, {}), value: size(server.limits.memory) },
    { label: t(`${base}.limit.disk`, {}), value: size(server.limits.disk) },
    { label: t(`${base}.limit.backups`, {}), value: count(usage?.backups, server.featureLimits.backups) },
    { label: t(`${base}.limit.databases`, {}), value: count(usage?.databases, server.featureLimits.databases) },
    {
      label: t(`${base}.limit.allocations`, {}),
      value: count(usage?.allocations, server.featureLimits.allocations),
    },
  ];

  return (
    <>
      <span className='theme-setting-caption'>{t(`${base}.limits`, {})}</span>
      <div className='theme-setting-limits'>
        {limits.map((limit) => (
          <div key={limit.label} className='theme-setting-limit'>
            <small>{limit.label}</small>
            <strong>{limit.value}</strong>
          </div>
        ))}
      </div>
    </>
  );
}

export default function ServerInformationContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const canRename = useServerCan('settings.rename');
  const resize = useResizeOptions();

  const [name, setName] = useState(server.name);
  const [description, setDescription] = useState(server.description ?? '');

  useEffect(() => setName(server.name), [server.name]);
  useEffect(() => setDescription(server.description ?? ''), [server.description]);

  const parsed = serverSettingsRenameSchema.safeParse({ name: name.trim(), description });
  const nameError = parsed.success ? undefined : parsed.error.issues.find((issue) => issue.path[0] === 'name')?.message;
  const descriptionError = parsed.success
    ? undefined
    : parsed.error.issues.find((issue) => issue.path[0] === 'description')?.message;

  const save = (field: 'name' | 'description') => {
    if (!parsed.success || !canRename) return;

    const values = parsed.data;
    const changed =
      field === 'name' ? values.name !== server.name : values.description !== (server.description || null);
    if (!changed) return;

    const next =
      field === 'name'
        ? { name: values.name, description: server.description }
        : { name: server.name, description: values.description };

    renameServer(server.uuid, next)
      .then(() => {
        addToast(t(field === 'name' ? `${base}.nameUpdated` : `${base}.descriptionUpdated`, {}), 'success');
        updateServer(next);
      })
      .catch((err) => {
        addToast(httpErrorToHuman(err), 'error');
        setName(server.name);
        setDescription(server.description ?? '');
      });
  };

  return (
    <TitleCard title={t(`${base}.title`, {})}>
      <Stack>
        <TextInput
          withAsterisk
          label={t(`${base}.name`, {})}
          value={name}
          error={nameError}
          disabled={!canRename}
          onChange={(e) => setName(e.currentTarget.value)}
          onBlur={() => save('name')}
        />
        <TextArea
          label={t(`${base}.description`, {})}
          autosize
          minRows={3}
          value={description}
          error={descriptionError}
          disabled={!canRename}
          onChange={(e) => setDescription(e.currentTarget.value)}
          onBlur={() => save('description')}
        />
      </Stack>

      <SettingRows className='mt-4'>
        <SettingRow
          label={t(`${base}.uuid`, {})}
          mono
          actions={<CopyActionIcon content={server.uuid} label={t(`${base}.copyUuid`, {})} />}
        >
          {server.uuid}
        </SettingRow>
        <SettingRow
          label={t(`${base}.identifier`, {})}
          mono
          actions={<CopyActionIcon content={server.uuidShort} label={t(`${base}.copyIdentifier`, {})} />}
        >
          {server.uuidShort}
        </SettingRow>
        <SettingRow label={t(`${base}.node`, {})}>
          {server.nodeName} <span className='theme-setting-value-muted font-mono'>#{server.nodeUuid.slice(0, 8)}</span>
        </SettingRow>
      </SettingRows>

      {/* as Pelican: the limits only for those who cannot change them (not while that is still loading) */}
      {!resize.usable && !(resize.resource.loading && !resize.resource.data) && <Limits />}
    </TitleCard>
  );
}
