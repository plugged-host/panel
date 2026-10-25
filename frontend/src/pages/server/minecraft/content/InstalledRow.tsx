// plugged.host: one installed plugin or mod and what can be done with it (docs/features/minecraft.md)
import { faArrowsRotate, faArrowUp, faDownload, faPause, faPlay, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteInstalledContent from '@/api/server/minecraft/deleteInstalledContent.ts';
import identifyInstalledContent from '@/api/server/minecraft/identifyInstalledContent.ts';
import toggleInstalledContent from '@/api/server/minecraft/toggleInstalledContent.ts';
import updateInstalledContent from '@/api/server/minecraft/updateInstalledContent.ts';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString } from '@/lib/format/size.ts';
import { minecraftInstalledJarSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ContentIcon from './ContentIcon.tsx';
import { minecraftSourceLabels } from './labels.ts';

interface Props {
  jar: z.infer<typeof minecraftInstalledJarSchema>;
  directory: string;
  onChanged: () => void;
}

export default function InstalledRow({ jar, directory, onChanged }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const canManage = useServerCan('minecraft.content');

  const [openModal, setOpenModal] = useState<'delete' | 'update' | null>(null);
  const [withData, setWithData] = useState(false);
  const [busy, setBusy] = useState<'toggle' | 'identify' | null>(null);
  const base = 'pages.server.minecraft.content.installed';

  const fail = (error: unknown) => {
    addToast(httpErrorToHuman(error), 'error');
    onChanged();
  };

  const toggle = () => {
    setBusy('toggle');
    toggleInstalledContent(server.uuid, jar.file, jar.disabled)
      .then(() =>
        addToast(
          jar.disabled
            ? t(`${base}.toast.enabled`, { name: jar.name })
            : t(`${base}.toast.disabled`, { name: jar.name }),
          'success',
        ),
      )
      .then(onChanged)
      .catch(fail)
      .finally(() => setBusy(null));
  };

  const identify = () => {
    setBusy('identify');
    identifyInstalledContent(server.uuid, jar.file)
      .then((identified) =>
        identified.source
          ? addToast(t(`${base}.toast.identified`, { name: identified.name }), 'success')
          : addToast(t(`${base}.identifyUnknown`, {}), 'warning'),
      )
      .then(onChanged)
      .catch(fail)
      .finally(() => setBusy(null));
  };

  const remove = () =>
    deleteInstalledContent(server.uuid, jar.file, withData)
      .then(() => {
        addToast(t(`${base}.toast.deleted`, { name: jar.name }), 'success');
        setOpenModal(null);
      })
      .then(onChanged)
      .catch(fail);

  const update = () =>
    updateInstalledContent(server.uuid, jar.file)
      .then((file) => {
        addToast(t(`${base}.toast.updating`, { file }), 'success');
        setOpenModal(null);
      })
      .then(onChanged)
      .catch(fail);

  // the Pelican panel's meta line: source (" ~" for a name match) · authors · file · size
  const sourceLabel = jar.source
    ? `${minecraftSourceLabels[jar.source]}${jar.confidence === 'name' ? ' ~' : ''}`
    : !jar.pending && jar.confidence === 'unknown'
      ? t(`${base}.unknown`, {})
      : null;
  const meta = [jar.authors.slice(0, 2).join(', '), jar.file, jar.size > 0 ? bytesToString(jar.size) : '']
    .filter(Boolean)
    .join(' · ');

  return (
    <div className='theme-jar-row'>
      <ConfirmationModal
        opened={openModal === 'delete'}
        onClose={() => setOpenModal(null)}
        title={t(`${base}.modal.delete.title`, { name: jar.name })}
        confirm={t('common.button.delete', {})}
        onConfirmed={remove}
      >
        <Stack gap='md'>
          <Text size='sm'>{t(`${base}.modal.delete.content`, { file: jar.file, directory })}</Text>
          {jar.dataFolder && (
            <Switch
              label={t(`${base}.modal.delete.withData`, { folder: jar.dataFolder })}
              description={t(`${base}.modal.delete.withDataDescription`, {})}
              checked={withData}
              onChange={(e) => setWithData(e.currentTarget.checked)}
            />
          )}
        </Stack>
      </ConfirmationModal>

      {jar.update && (
        <ConfirmationModal
          opened={openModal === 'update'}
          onClose={() => setOpenModal(null)}
          title={t(`${base}.modal.update.title`, { name: jar.name, version: jar.update.number })}
          confirm={t('pages.server.minecraft.content.button.confirm', {})}
          confirmColor='blue'
          onConfirmed={update}
        >
          <Text size='sm'>{t(`${base}.modal.update.content`, {})}</Text>
        </ConfirmationModal>
      )}

      <div className='theme-jar-icon'>
        {jar.pending ? <Spinner size={20} /> : <ContentIcon icon={jar.icon} kind={jar.kind} variant='jar' />}
      </div>

      <div className='theme-jar-main'>
        <div className='theme-jar-line'>
          <span className='theme-jar-name'>
            {jar.url ? (
              <a href={jar.url} target='_blank' rel='noopener noreferrer'>
                {jar.name}
              </a>
            ) : (
              jar.name
            )}
          </span>
          {jar.version && <span className='theme-jar-version'>{jar.version}</span>}
          {jar.disabled && (
            <Badge color='gray' size='sm'>
              {t(`${base}.badge.disabled`, {})}
            </Badge>
          )}
          {jar.update && (
            <Tooltip label={t(`${base}.button.updateTo`, { version: jar.update.number })}>
              <Badge color='blue' size='sm' leftSection={<FontAwesomeIcon icon={faArrowUp} />}>
                {jar.update.number}
              </Badge>
            </Tooltip>
          )}
          {jar.missing.map((dependency) => (
            <Tooltip key={dependency} label={t(`${base}.missingHint`, { name: dependency })}>
              <Badge color='red' size='sm'>
                {t(`${base}.badge.missing`, { name: dependency })}
              </Badge>
            </Tooltip>
          ))}
        </div>
        <div className='theme-jar-meta' title={jar.description ?? undefined}>
          {sourceLabel && (
            <span title={jar.source ? t(`${base}.confidence.${jar.confidence}`, {}) : t(`${base}.identifyUnknown`, {})}>
              {sourceLabel}
            </span>
          )}
          {sourceLabel && meta ? ' · ' : ''}
          {meta}
        </div>
      </div>

      {canManage && !jar.pending && (
        <div className='theme-jar-actions'>
          {jar.update && (
            <Tooltip label={t(`${base}.button.updateTo`, { version: jar.update.number })}>
              <ActionIcon
                size='input-sm'
                variant='subtle'
                color='blue'
                aria-label={t(`${base}.button.updateTo`, { version: jar.update.number })}
                onClick={() => setOpenModal('update')}
              >
                <FontAwesomeIcon icon={faDownload} />
              </ActionIcon>
            </Tooltip>
          )}
          <Tooltip label={jar.disabled ? t('common.button.enable', {}) : t('common.button.disable', {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              aria-label={jar.disabled ? t('common.button.enable', {}) : t('common.button.disable', {})}
              loading={busy === 'toggle'}
              onClick={toggle}
            >
              <FontAwesomeIcon icon={jar.disabled ? faPlay : faPause} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label={t('common.button.delete', {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='red'
              aria-label={t('common.button.delete', {})}
              onClick={() => {
                setWithData(false);
                setOpenModal('delete');
              }}
            >
              <FontAwesomeIcon icon={faTrash} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label={t(`${base}.button.identify`, {})}>
            <ActionIcon
              size='input-sm'
              variant='subtle'
              color='gray'
              aria-label={t(`${base}.button.identify`, {})}
              loading={busy === 'identify'}
              onClick={identify}
            >
              <FontAwesomeIcon icon={faArrowsRotate} />
            </ActionIcon>
          </Tooltip>
        </div>
      )}
    </div>
  );
}
