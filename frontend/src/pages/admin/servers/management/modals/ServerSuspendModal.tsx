// plugged.host: suspending asks why (shown to the owner) and, optionally, when the server goes
// (docs/features/suspension.md).
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { FormEvent, useEffect, useState } from 'react';
import updateServer from '@/api/admin/servers/updateServer.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import FormModal from '@/elements/modals/FormModal.tsx';
import { ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { AdminServer } from '@/lib/schemas/admin/servers.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

const DELETE_AFTER = [
  { value: 'never', days: null, key: 'never' },
  { value: '1', days: 1, key: 'oneDay' },
  { value: '3', days: 3, key: 'threeDays' },
  { value: '7', days: 7, key: 'sevenDays' },
  { value: '30', days: 30, key: 'thirtyDays' },
] as const;

export default function ServerSuspendModal({ server, ...props }: ModalProps & { server: AdminServer }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();

  const [loading, setLoading] = useState(false);
  const [reason, setReason] = useState('');
  const [deleteAfter, setDeleteAfter] = useState<string>('never');

  useEffect(() => {
    if (!props.opened) {
      setReason('');
      setDeleteAfter('never');
    }
  }, [props.opened]);

  const doSuspend = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading(true);

    const days = DELETE_AFTER.find((option) => option.value === deleteAfter)?.days ?? null;

    updateServer(server.uuid, {
      suspended: true,
      suspensionReason: reason.trim() || null,
      suspensionDeleteAt: days === null ? null : new Date(Date.now() + days * 86_400_000),
    })
      .then(() => {
        addToast(t('pages.admin.servers.tabs.management.page.suspend.toast.suspended', {}), 'success');
        props.onClose();
        queryClient.invalidateQueries({ queryKey: queryKeys.admin.servers.all() });
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <FormModal
      title={t('pages.admin.servers.tabs.management.page.suspend.modal.title', {})}
      loading={loading}
      {...props}
      onSubmit={doSuspend}
    >
      <Stack>
        <Text size='sm'>
          {t('pages.admin.servers.tabs.management.page.suspend.modal.content', { name: server.name }).md()}
        </Text>

        <TextArea
          label={t('pages.admin.servers.tabs.management.page.suspend.modal.form.reason', {})}
          description={t('pages.admin.servers.tabs.management.page.suspend.modal.form.reasonDescription', {})}
          maxLength={1000}
          autosize
          minRows={3}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
        />

        <Select
          label={t('pages.admin.servers.tabs.management.page.suspend.modal.form.deleteAfter', {})}
          description={t('pages.admin.servers.tabs.management.page.suspend.modal.form.deleteAfterDescription', {})}
          data={DELETE_AFTER.map((option) => ({
            value: option.value,
            label: t(`pages.admin.servers.tabs.management.page.suspend.modal.form.deleteOptions.${option.key}`, {}),
          }))}
          allowDeselect={false}
          value={deleteAfter}
          onChange={(value) => setDeleteAfter(value ?? 'never')}
        />
      </Stack>

      <ModalFooter>
        <Button color='red' type='submit' loading={loading}>
          {t('pages.admin.servers.tabs.management.page.suspend.button', {})}
        </Button>
        <Button variant='default' onClick={() => props.onClose()}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </FormModal>
  );
}
