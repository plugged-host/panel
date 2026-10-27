// plugged.host: Settings' "Server Information" box: the server's icon (its own, else its game's) with Upload
// Icon and Delete Icon, its name and description, each saved when it loses focus, then its UUID, identifier
// and node as rows (docs/features/server-settings.md#server-information)
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import removeServerIcon from '@/api/server/settings/removeServerIcon.ts';
import renameServer from '@/api/server/settings/renameServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { CopyActionIcon, SettingRow, SettingRows } from '@/elements/SettingRows.tsx';
import { serverSettingsRenameSchema } from '@/lib/schemas/server/settings.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ServerIconModal from './modals/ServerIconModal.tsx';
import SettingsSection from './SettingsSection.tsx';

const base = 'pages.server.settings.general';

// the icon with its two buttons: Upload Icon, and Delete Icon for an icon of the server's own (no
// confirmation: the game's comes back). Nothing when there is no picture and no permission to add one.
function IconRow() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const canChange = useServerCan('settings.change-icon');

  const [uploading, setUploading] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const src = server.icon ?? server.eggIcon;
  if (!src && !canChange) {
    return null;
  }

  const doDelete = () => {
    setDeleting(true);
    removeServerIcon(server.uuid)
      .then(() => {
        updateServer({ icon: null });
        addToast(t(`${base}.icon.deleted`, {}), 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setDeleting(false));
  };

  return (
    <div className='flex flex-wrap items-center gap-4'>
      {src && <img className='size-16 shrink-0 rounded-xl object-cover' src={src} alt='' width={64} height={64} />}
      <div className='flex flex-col gap-2'>
        {!server.icon && src && <p className='theme-eco-muted'>{t(`${base}.icon.tooltip`, {})}</p>}
        {canChange && (
          <div className='flex flex-wrap gap-3'>
            <Button variant='default' size='sm' onClick={() => setUploading(true)}>
              {t(`${base}.icon.upload`, {})}
            </Button>
            {server.icon && (
              <Button variant='default' size='sm' loading={deleting} onClick={doDelete}>
                {t(`${base}.icon.delete`, {})}
              </Button>
            )}
          </div>
        )}
      </div>

      {canChange && <ServerIconModal opened={uploading} onClose={() => setUploading(false)} />}
    </div>
  );
}

export default function InformationSection() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const canRename = useServerCan('settings.rename');
  const nodeId = server.nodeUuid.slice(0, 8);

  const [name, setName] = useState(server.name);
  const [description, setDescription] = useState(server.description ?? '');

  useEffect(() => setName(server.name), [server.name]);
  useEffect(() => setDescription(server.description ?? ''), [server.description]);

  const parsed = serverSettingsRenameSchema.safeParse({ name: name.trim(), description });
  const issue = (field: 'name' | 'description') =>
    parsed.success ? undefined : parsed.error.issues.find((item) => item.path[0] === field)?.message;

  // each field saves on its own when it loses focus, and only when it changed
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
    <SettingsSection id='general' title={t(`${base}.title`, {})}>
      <div className='theme-eco-card'>
        <IconRow />

        <div className='grid grid-cols-1 gap-4 md:grid-cols-2'>
          <TextInput
            withAsterisk
            label={t(`${base}.name`, {})}
            value={name}
            error={issue('name')}
            disabled={!canRename}
            onChange={(e) => setName(e.currentTarget.value)}
            onBlur={() => save('name')}
          />
          <TextArea
            label={t(`${base}.description`, {})}
            autosize
            minRows={1}
            value={description}
            error={issue('description')}
            disabled={!canRename}
            onChange={(e) => setDescription(e.currentTarget.value)}
            onBlur={() => save('description')}
          />
        </div>

        <SettingRows>
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
            {server.nodeName} <span className='theme-setting-value-muted font-mono'>#{nodeId}</span>
          </SettingRow>
        </SettingRows>
      </div>
    </SettingsSection>
  );
}
