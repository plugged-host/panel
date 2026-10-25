import { ModalProps } from '@mantine/core';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import installServer from '@/api/server/settings/installServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx'; // plugged.host
import { serverSettingsReinstallSchema } from '@/lib/schemas/server/settings.ts';
import { useModalForm } from '@/plugins/form/useModalForm.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function SettingsReinstallModal({ ...props }: ModalProps) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const navigate = useNavigate();
  const { user } = useAuth();
  // plugged.host: only admins start after a reinstall, the start would skip the start gates
  const canStart = useServerCan('control.start') && !!user?.admin;

  const { form, handleClose, handleSubmit, loading, isDirty } = useModalForm<
    z.infer<typeof serverSettingsReinstallSchema>
  >({
    initialValues: {
      truncateDirectory: false,
      startOnCompletion: false,
    },
    schema: serverSettingsReinstallSchema,
    onClose: props.onClose,
    onSubmit: async (values) => {
      await installServer(server.uuid, values);
      addToast(t('pages.server.settings.danger.reinstallStarted', {}), 'success'); // plugged.host: Pelican's toast
      navigate(`/server/${server.uuidShort}`);
      updateServer({ status: 'installing' });
    },
  });

  return (
    <FormModal
      title={t('pages.server.settings.danger.reinstallModalTitle', {})} // plugged.host: Pelican's wording
      isDirty={isDirty}
      loading={loading}
      {...props}
      onClose={handleClose}
      onSubmit={handleSubmit}
    >
      <Stack>
        {/* plugged.host: Pelican's warning above the switches */}
        <Text size='sm' c='dimmed'>
          {t('pages.server.settings.danger.reinstallModalBody', {})}
        </Text>
        <Switch
          label={t('common.form.truncateDirectory', {})}
          name='truncate'
          {...form.getInputProps('truncateDirectory', { type: 'checkbox' })}
        />
        {canStart && (
          <Switch
            label={t('pages.server.settings.reinstall.modal.startOnCompletion', {})}
            name='startOnCompletion'
            {...form.getInputProps('startOnCompletion', { type: 'checkbox' })}
          />
        )}
      </Stack>

      <ModalFooter>
        <Button color='red' type='submit' loading={loading} disabled={!form.isValid()}>
          {/* plugged.host: Pelican's wording */}
          {t('pages.server.settings.danger.reinstallConfirm', {})}
        </Button>
        <Button variant='default' onClick={handleClose}>
          {/* plugged.host */}
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </FormModal>
  );
}
