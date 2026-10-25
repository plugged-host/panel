// plugged.host: deleting the server, from the settings header and the Danger Zone. Pelican's
// wording, and the server's name typed to confirm (docs/features/server-settings.md)
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteServer from '@/api/server/deleteServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function DeleteServerModal({ opened, onClose }: Pick<ModalProps, 'opened' | 'onClose'>) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);

  const [typed, setTyped] = useState('');
  const [loading, setLoading] = useState(false);

  const close = () => {
    setTyped('');
    onClose();
  };

  const doDelete = () => {
    setLoading(true);
    deleteServer(server.uuid)
      .then(() => {
        addToast(t('pages.server.settings.delete.toast.deleted', {}), 'success');
        navigate('/');
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t('pages.server.settings.delete.modal.title', {})} opened={opened} onClose={close}>
      <Stack>
        <Text size='sm' c='dimmed'>
          {t('pages.server.settings.delete.modal.content', {})}
        </Text>
        <TextInput
          label={t('pages.server.settings.delete.modal.typeName', {})}
          placeholder={server.name}
          value={typed}
          onChange={(e) => setTyped(e.currentTarget.value)}
        />
      </Stack>

      <ModalFooter>
        <Button color='red' loading={loading} disabled={typed.trim() !== server.name} onClick={doDelete}>
          {t('pages.server.settings.delete.modal.button', {})}
        </Button>
        <Button variant='default' onClick={close}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
