// plugged.host: a failover drill: one server of the node restored on another node from its newest
// backup (docs/features/failover.md).
import { ModalProps } from '@mantine/core';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import getNodeServers from '@/api/admin/nodes/servers/getNodeServers.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import ServerSelect from '@/elements/input/ServerSelect.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminNodeSchema } from '@/lib/schemas/admin/nodes.ts';
import { adminServerSchema } from '@/lib/schemas/admin/servers.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Props = ModalProps & {
  node: z.infer<typeof adminNodeSchema>;
  onConfirmed: (serverUuid: string) => Promise<void>;
};

export default function FailoverDrillModal({ node, onConfirmed, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const modal = 'pages.admin.nodes.tabs.failover.page.modal.drill';

  const [serverUuid, setServerUuid] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!props.opened) {
      setServerUuid(null);
    }
  }, [props.opened]);

  const doDrill = () => {
    if (!serverUuid) {
      return;
    }

    setLoading(true);

    onConfirmed(serverUuid)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${modal}.title`, {})} {...props}>
      <Stack>
        {t(`${modal}.content`, {}).md()}

        <ServerSelect<z.infer<typeof adminServerSchema>>
          withAsterisk
          label={t('common.form.server', {})}
          placeholder={t('common.form.server', {})}
          queryKey={queryKeys.admin.nodes.servers(node.uuid)}
          fetcher={(search) => getNodeServers(node.uuid, 1, search)}
          value={serverUuid}
          onChange={(uuid) => setServerUuid(uuid)}
        />

        <Alert color='yellow'>{t(`${modal}.warning`, {})}</Alert>

        <ModalFooter>
          <Button color='red' onClick={doDrill} loading={loading} disabled={!serverUuid}>
            {t('pages.admin.nodes.tabs.failover.page.button.drill', {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
