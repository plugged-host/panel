// plugged.host: replace the loaded world with one from an archive (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import FileInput from '@/elements/input/FileInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { uploadWorld } from './worldUpload.ts';

const ARCHIVES = ['.zip', '.tar', '.tar.gz', '.tgz', '.tar.xz', '.tar.zst'];

export default function UploadWorldModal(props: ModalProps) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.worlds';

  const [file, setFile] = useState<File | null>(null);
  const [keep, setKeep] = useState(true);
  const [loading, setLoading] = useState(false);

  const close = () => {
    setFile(null);
    setKeep(true);
    props.onClose();
  };

  const valid =
    !!file && !file.name.startsWith('.') && ARCHIVES.some((extension) => file.name.toLowerCase().endsWith(extension));

  const submit = () => {
    if (!file) return;
    setLoading(true);

    uploadWorld(server, file, keep, (error) => {
      if (error) {
        addToast(httpErrorToHuman(error), 'error');
      } else {
        addToast(t(`${base}.toast.importStarted`, { file: file.name }), 'success');
      }
      queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });
    })
      .then(() => {
        addToast(t(`${base}.toast.uploading`, { file: file.name }), 'success');
        close();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${base}.modal.upload.title`, {})} {...props} onClose={close}>
      <Stack>
        <Text size='sm'>{t(`${base}.modal.upload.content`, {})}</Text>

        <FileInput
          withAsterisk
          label={t(`${base}.form.archive`, {})}
          description={t(`${base}.form.archiveDescription`, {})}
          accept={ARCHIVES.join(',')}
          value={file}
          onChange={setFile}
          error={file && !valid ? t(`${base}.form.archiveInvalid`, {}) : undefined}
        />
        <Checkbox
          label={t(`${base}.form.keep`, {})}
          description={t(`${base}.form.keepRenamed`, {})}
          checked={keep}
          onChange={(e) => setKeep(e.currentTarget.checked)}
        />

        <ModalFooter>
          <Button onClick={submit} loading={loading} disabled={!valid}>
            {t(`${base}.button.uploadSubmit`, {})}
          </Button>
          <Button variant='default' onClick={close}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
