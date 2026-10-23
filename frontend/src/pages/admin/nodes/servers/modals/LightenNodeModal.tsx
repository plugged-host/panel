// plugged.host: "Lighten Load": queue moves of some of the node's servers to the other nodes of its
// location (docs/features/transfers.md).
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import { z } from 'zod';
import lightenNode from '@/api/admin/nodes/lightenNode.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { adminLightenNodeSchema } from '@/lib/schemas/admin/capacity.ts';
import { AdminNode } from '@/lib/schemas/admin/nodes.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Pick = z.infer<typeof adminLightenNodeSchema>['pick'];

export default function LightenNodeModal({ node, ...props }: ModalProps & { node: AdminNode }) {
  const { t, tItem } = useTranslations();
  const { addToast } = useToast();
  const base = 'pages.admin.nodes.tabs.servers.page.modal.lighten';

  const [pick, setPick] = useState<Pick>('stopped');
  const [count, setCount] = useState(5);
  const [loading, setLoading] = useState(false);

  const doLighten = () => {
    setLoading(true);

    lightenNode(node.uuid, { pick, count })
      .then((queued) => {
        addToast(
          queued > 0 ? t(`${base}.toast.queued`, { servers: tItem('server', queued) }) : t(`${base}.toast.none`, {}),
          queued > 0 ? 'success' : 'warning',
        );
        props.onClose();
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${base}.title`, {})} {...props}>
      <Stack>
        <Text size='sm' c='dimmed'>
          {t(`${base}.description`, { location: node.location.name })}
        </Text>

        <Select
          withAsterisk
          label={t(`${base}.form.pick`, {})}
          value={pick}
          onChange={(value) => setPick((value as Pick) ?? 'stopped')}
          data={[
            { value: 'stopped', label: t(`${base}.enum.pick.stopped`, {}) },
            { value: 'busiest', label: t(`${base}.enum.pick.busiest`, {}) },
          ]}
        />

        <NumberInput
          withAsterisk
          label={t(`${base}.form.count`, {})}
          min={1}
          max={100}
          value={count}
          onChange={(value) => setCount(Math.min(Math.max(Number(value) || 1, 1), 100))}
        />
      </Stack>

      <ModalFooter>
        <Button color='blue' loading={loading} onClick={doLighten}>
          {t(`${base}.button`, {})}
        </Button>
        <Button variant='default' onClick={props.onClose}>
          {t('common.button.cancel', {})}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
