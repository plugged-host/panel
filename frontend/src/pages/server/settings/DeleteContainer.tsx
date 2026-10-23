// plugged.host: an owner deleting their server, when the panel allows it (docs/features/server-settings.md)
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { httpErrorToHuman } from '@/api/axios.ts';
import deleteServer from '@/api/server/deleteServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

export default function DeleteContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);
  const allowDeleting = useGlobalStore((state) => state.settings.games.allowDeleting);

  const [confirming, setConfirming] = useState(false);
  const [typed, setTyped] = useState('');
  const [loading, setLoading] = useState(false);

  if (!allowDeleting && !user?.admin) {
    return null;
  }

  const close = () => {
    setConfirming(false);
    setTyped('');
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
    <TitleCard title={t('pages.server.settings.delete.title', {})} className='h-full order-70'>
      <Modal title={t('pages.server.settings.delete.modal.title', {})} opened={confirming} onClose={close}>
        <Stack>
          {t('pages.server.settings.delete.modal.content', { name: server.name }).md()}
          <TextInput
            label={t('pages.server.settings.delete.modal.typeName', {})}
            placeholder={server.name}
            value={typed}
            onChange={(e) => setTyped(e.currentTarget.value)}
          />
        </Stack>

        <ModalFooter>
          <Button color='red' loading={loading} disabled={typed.trim() !== server.name} onClick={doDelete}>
            {t('common.button.delete', {})}
          </Button>
          <Button variant='default' onClick={close}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Modal>

      <Stack h='100%'>
        {t('pages.server.settings.delete.content', {}).md()}

        <Group mt='auto'>
          <Button variant='default' onClick={() => setConfirming(true)}>
            {t('pages.server.settings.delete.button', {})}
          </Button>
        </Group>
      </Stack>
    </TitleCard>
  );
}
