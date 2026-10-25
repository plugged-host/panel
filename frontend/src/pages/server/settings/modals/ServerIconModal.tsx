// plugged.host: uploading the server's icon, the Pelican panel's "Upload Icon" (UploadIcon's File
// tab): a picture cut to a square in upstream's avatar editor (docs/features/server-settings.md#server-icon)
import { ModalProps } from '@mantine/core';
import { useRef, useState } from 'react';
import AvatarEditor, { AvatarEditorRef } from 'react-avatar-editor';
import { httpErrorToHuman } from '@/api/axios.ts';
import updateServerIcon from '@/api/server/settings/updateServerIcon.ts';
import Button from '@/elements/buttons/Button.tsx';
import FileInput from '@/elements/input/FileInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

const base = 'pages.server.settings.general.icon';

/** The formats the panel takes (shared/src/models/user/avatar.rs `decode`). */
const ACCEPT = 'image/png,image/jpeg,image/webp,image/gif';

export default function ServerIconModal({ opened, onClose }: Pick<ModalProps, 'opened' | 'onClose'>) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);

  const [file, setFile] = useState<File | null>(null);
  const [loading, setLoading] = useState(false);
  const editor = useRef<AvatarEditorRef>(null);

  const close = () => {
    setFile(null);
    onClose();
  };

  const upload = (icon: Blob) =>
    updateServerIcon(server.uuid, icon)
      .then((url) => {
        updateServer({ icon: url });
        addToast(t(`${base}.updated`, {}), 'success');
        close();
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setLoading(false));

  const doUpload = () => {
    if (!file || loading) return;

    setLoading(true);
    editor.current?.getImageScaledToCanvas().toBlob(
      (blob) => {
        if (blob) {
          upload(blob);
        } else {
          setLoading(false);
          addToast(t('pages.account.account.containers.avatar.toast.loadFailed', {}), 'error');
        }
      },
      'image/webp',
      0.9,
    );
  };

  return (
    <Modal title={t(`${base}.upload`, {})} opened={opened} onClose={close}>
      <Stack>
        <FileInput
          label={t(`${base}.file`, {})}
          value={file}
          onChange={setFile}
          accept={ACCEPT}
          disabled={loading}
          clearable
        />

        {file && (
          <div className='flex justify-center'>
            <AvatarEditor
              key={`${file.name}-${file.lastModified}`}
              ref={editor}
              image={file}
              width={256}
              height={256}
              showGrid
              onLoadFailure={() => {
                setFile(null);
                addToast(t('pages.account.account.containers.avatar.toast.loadFailed', {}), 'error');
              }}
              style={{ width: 256, height: 256, borderRadius: '0.75rem' }}
            />
          </div>
        )}
      </Stack>

      <ModalFooter>
        <Button loading={loading} disabled={!file} onClick={doUpload}>
          {t(`${base}.upload`, {})}
        </Button>
        <Button variant='default' onClick={close}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
