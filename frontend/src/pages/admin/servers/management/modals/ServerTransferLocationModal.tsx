// plugged.host: Admin › Servers › Transfer to the best node of a location, through the moves players make: ports kept where free, backups along, retried on another node (docs/features/transfers.md).
import { ModalProps } from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import getLocation from '@/api/admin/locations/getLocation.ts';
import getLocations from '@/api/admin/locations/getLocations.ts';
import postTransferLocation from '@/api/admin/servers/postTransferLocation.ts';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminLocationSchema } from '@/lib/schemas/admin/locations.ts';
import { AdminServer } from '@/lib/schemas/admin/servers.ts';
import { useSearchableResource } from '@/plugins/resource/useSearchableResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';

/** The destination that keeps the upstream fields: a node and its allocations chosen by hand. */
export const TRANSFER_BY_HAND = 'node';

const form = 'pages.admin.servers.tabs.management.page.transfer.modal.form';

/** Where the server goes: the best node of a location (its own: another node there), or by hand. */
export function TransferTargetSelect({
  server,
  value,
  onChange,
}: {
  server: AdminServer;
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslations();

  const locations = useSearchableResource<z.infer<typeof adminLocationSchema>>({
    queryKey: queryKeys.admin.locations.all(),
    fetcher: (search) => getLocations(1, search),
  });

  const own = server.node.location;

  return (
    <Select
      withAsterisk
      label={t(`${form}.target`, {})}
      description={t(`${form}.targetDescription`, {})}
      value={value}
      onChange={(target) => onChange(target || TRANSFER_BY_HAND)}
      data={[
        { value: own.uuid, label: t(`${form}.targetOwnLocation`, { location: own.name }) },
        ...locations.items
          .filter((location) => location.uuid !== own.uuid)
          .map((location) => ({
            value: location.uuid,
            label: t(`${form}.targetLocation`, { location: location.name }),
          })),
        { value: TRANSFER_BY_HAND, label: t(`${form}.targetNode`, {}) },
      ]}
      searchable
      searchValue={locations.search}
      onSearchChange={locations.setSearch}
      loading={locations.loading}
    />
  );
}

type Props = ModalProps & {
  server: AdminServer;
  target: string;
  onTargetChange: (value: string) => void;
};

export default function ServerTransferLocationModal({ server, target, onTargetChange, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const transferEnabled = useGlobalStore((state) => !!state.settings.games?.transferEnabled);
  const transfer = 'pages.admin.servers.tabs.management.page.transfer';

  const [confirming, setConfirming] = useState(false);
  const [start, setStart] = useState(false);

  const location = useQuery({
    queryKey: queryKeys.admin.locations.detail(target),
    queryFn: () => getLocation(target),
    enabled: target !== server.node.location.uuid,
  });

  const doTransfer = async () => {
    await postTransferLocation(server.uuid, { locationUuid: target, start })
      .then(() => {
        addToast(t(`${transfer}.toast.started`, {}), 'success');
        setConfirming(false);
        props.onClose();
        // the move's progress: the server's Transfer page, when players may move servers
        navigate(transferEnabled ? `/server/${server.uuidShort}/transfer` : `/server/${server.uuidShort}`);
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'));
  };

  return (
    <>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${transfer}.modal.confirm.title`, {})}
        confirm={t('common.button.transfer', {})}
        onConfirmed={doTransfer}
      >
        {t(`${transfer}.modal.confirm.contentLocation`, {
          name: server.name,
          from: server.node.name,
          location: target === server.node.location.uuid ? server.node.location.name : (location.data?.name ?? ''),
        }).md()}
      </ConfirmationModal>

      <Modal title={t(`${transfer}.modal.title`, {})} {...props} opened={props.opened && !confirming}>
        <Stack>
          <TransferTargetSelect server={server} value={target} onChange={onTargetChange} />

          <Switch
            label={t(`${form}.start`, {})}
            description={t(`${form}.startDescription`, {})}
            checked={start}
            onChange={(e) => setStart(e.target.checked)}
          />

          <Text size='sm' c='dimmed'>
            {t(`${form}.locationInfo`, {})}
          </Text>
        </Stack>

        <ModalFooter>
          <Button color='blue' onClick={() => setConfirming(true)}>
            {t('common.button.transfer', {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Modal>
    </>
  );
}
