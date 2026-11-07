import { faFile, faFolder } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ModalProps } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Card from '@/elements/data-display/Card.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import SegmentedControl from '@/elements/layout/SegmentedControl.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Text from '@/elements/typography/Text.tsx';
import { generateUploadName, UploadConflict, UploadConflictResolutions } from '@/lib/files/uploadConflicts.ts';
import { bytesToString } from '@/lib/format/size.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Resolution = 'skip' | 'overwrite' | 'rename';

type Props = ModalProps & {
  directory: string;
  conflicts: UploadConflict[];
  remaining: number;
  onResolve: (resolutions: UploadConflictResolutions) => void;
};

function Meta({ label, size, modified }: { label: string; size: number; modified: number | Date }) {
  const { t } = useTranslations();

  return (
    <div className='flex flex-col grow min-w-0'>
      <Text size='xs' fw={600} c='dimmed'>
        {label}
      </Text>
      <span className='flex flex-row items-center justify-between gap-2'>
        <Text size='sm' c='dimmed'>
          {t('pages.server.files.modal.details.logicalSize', {})}
        </Text>
        <Text size='sm'>{bytesToString(size)}</Text>
      </span>
      <span className='flex flex-row items-center justify-between gap-2'>
        <Text size='sm' c='dimmed'>
          {t('pages.server.files.modal.details.lastModifiedAt', {})}
        </Text>
        <Text size='sm'>
          <FormattedTimestamp timestamp={modified} />
        </Text>
      </span>
    </div>
  );
}

export default function UploadConflictModal({ directory, conflicts, remaining, onResolve, ...props }: Props) {
  const { t, tItem } = useTranslations();
  const [resolutions, setResolutions] = useState<Record<string, Resolution>>({});
  const [renames, setRenames] = useState<Record<string, string>>({});

  useEffect(() => {
    if (props.opened) {
      setResolutions(Object.fromEntries(conflicts.map((conflict) => [conflict.path, 'skip' as Resolution])));
      setRenames(Object.fromEntries(conflicts.map((conflict) => [conflict.path, generateUploadName(conflict.path)])));
    }
  }, [props.opened, conflicts]);

  const setAll = (resolution: Resolution) => {
    setResolutions(Object.fromEntries(conflicts.map((conflict) => [conflict.path, resolution])));
  };

  const invalidRename = useMemo(
    () =>
      conflicts.some(
        (conflict) =>
          resolutions[conflict.path] === 'rename' &&
          (!renames[conflict.path]?.trim() || renames[conflict.path].includes('/')),
      ),
    [conflicts, resolutions, renames],
  );

  const actionCount = useMemo(
    () =>
      conflicts.filter(
        (conflict) => resolutions[conflict.path] === 'overwrite' || resolutions[conflict.path] === 'rename',
      ).length,
    [conflicts, resolutions],
  );

  const handleApply = () => {
    const result: UploadConflictResolutions = { overwrite: [], rename: {} };

    for (const conflict of conflicts) {
      const resolution = resolutions[conflict.path];
      if (resolution === 'overwrite') {
        result.overwrite.push(conflict.path);
      } else if (resolution === 'rename') {
        result.rename[conflict.path] = renames[conflict.path].trim();
      }
    }

    onResolve(result);
  };

  const folders = conflicts.filter((conflict) => conflict.kind === 'folder').length;

  return (
    <Modal title={t('elements.fileUpload.modal.uploadConflict.title', {})} size='lg' {...props}>
      <Stack gap='sm'>
        <Group justify='space-between'>
          <Text size='sm' c='dimmed'>
            {t('elements.fileUpload.modal.uploadConflict.description', {
              items: folders === conflicts.length ? tItem('directory', folders) : tItem('file', conflicts.length),
              directory,
            }).md()}
          </Text>
          <Group gap='xs'>
            <Button size='compact-xs' variant='default' onClick={() => setAll('skip')}>
              {t('elements.fileUpload.modal.uploadConflict.skipAll', {})}
            </Button>
            <Button size='compact-xs' variant='default' onClick={() => setAll('overwrite')}>
              {t('elements.fileUpload.modal.uploadConflict.overwriteAll', {})}
            </Button>
          </Group>
        </Group>

        {remaining > 0 && (
          <Text size='sm' c='dimmed'>
            {t('elements.fileUpload.modal.uploadConflict.alsoUploading', { files: tItem('file', remaining) })}
          </Text>
        )}

        <div className='max-h-[50vh] min-h-0 overflow-y-auto'>
          <Stack gap='xs'>
            {conflicts.map((conflict) => (
              <Card key={conflict.path} className='p-3'>
                <Stack gap='xs'>
                  <Text fw={500} className='break-all'>
                    <FontAwesomeIcon
                      className='mr-2 text-(--mantine-color-dimmed)'
                      icon={conflict.kind === 'folder' ? faFolder : faFile}
                    />
                    {conflict.path}
                  </Text>
                  <div className='flex flex-col md:flex-row gap-3'>
                    {conflict.kind === 'file' ? (
                      <Meta
                        label={t('elements.fileUpload.modal.uploadConflict.source', {})}
                        size={conflict.file.size}
                        modified={conflict.file.lastModified}
                      />
                    ) : (
                      <div className='flex flex-col grow min-w-0'>
                        <Text size='xs' fw={600} c='dimmed'>
                          {t('elements.fileUpload.modal.uploadConflict.source', {})}
                        </Text>
                        <Text size='sm'>
                          {t('elements.fileUpload.modal.uploadConflict.selectedFolder', {
                            files: tItem('file', conflict.files),
                          })}
                        </Text>
                      </div>
                    )}
                    <Meta
                      label={t('elements.fileUpload.modal.uploadConflict.destination', {})}
                      size={conflict.entry.size}
                      modified={conflict.entry.modified}
                    />
                  </div>
                  <SegmentedControl
                    fullWidth
                    size='xs'
                    value={resolutions[conflict.path] ?? 'skip'}
                    onChange={(value) => setResolutions((prev) => ({ ...prev, [conflict.path]: value as Resolution }))}
                    data={[
                      { value: 'skip', label: t('elements.fileUpload.modal.uploadConflict.skip', {}) },
                      {
                        value: 'overwrite',
                        label:
                          conflict.kind === 'folder'
                            ? t('elements.fileUpload.modal.uploadConflict.merge', {})
                            : t('elements.fileUpload.modal.uploadConflict.overwrite', {}),
                      },
                      { value: 'rename', label: t('elements.fileUpload.modal.uploadConflict.rename', {}) },
                    ]}
                  />
                  {conflict.kind === 'folder' && resolutions[conflict.path] === 'overwrite' && (
                    <Text size='xs' c='dimmed'>
                      {t('elements.fileUpload.modal.uploadConflict.mergeHint', {
                        files: tItem('file', conflict.files),
                      })}
                    </Text>
                  )}
                  {resolutions[conflict.path] === 'rename' && (
                    <TextInput
                      size='xs'
                      label={t('common.form.newName', {})}
                      value={renames[conflict.path] ?? ''}
                      onChange={(e) => setRenames((prev) => ({ ...prev, [conflict.path]: e.target.value }))}
                    />
                  )}
                </Stack>
              </Card>
            ))}
          </Stack>
        </div>
      </Stack>

      <ModalFooter>
        <Button disabled={actionCount === 0 || invalidRename} onClick={handleApply}>
          {t('elements.fileUpload.modal.uploadConflict.confirm', { files: tItem('file', actionCount + remaining) })}
        </Button>
        <Button variant='default' onClick={props.onClose}>
          {t('common.button.close', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
