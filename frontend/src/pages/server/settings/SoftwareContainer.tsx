// plugged.host: switch the server's software, which reinstalls it (docs/features/server-settings.md)
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import switchServerSoftware from '@/api/server/settings/switchServerSoftware.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Select from '@/elements/input/Select.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function SoftwareContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.software(),
    queryFn: () => getServerSoftware(server.uuid),
    silent: true,
  });
  const software = resource.data;

  const [variant, setVariant] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (software) setVariant(software.current);
  }, [software]);

  if (!software || software.variants.length < 2) {
    return null;
  }

  const label = software.variants.find((choice) => choice.value === variant)?.label ?? '';

  const doSwitch = async () => {
    if (!variant) return;

    await switchServerSoftware(server.uuid, { variant });
    addToast(t('pages.server.settings.software.toast.switching', {}), 'success');
    setConfirming(false);
    resource.invalidate();
    navigate(`/server/${server.uuidShort}`);
    updateServer({ status: 'installing' });
  };

  return (
    <TitleCard title={t('pages.server.settings.software.title', {})} className='h-full order-55'>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t('pages.server.settings.software.modal.title', {})}
        confirm={t('pages.server.settings.software.modal.button', {})}
        onConfirmed={doSwitch}
      >
        {t('pages.server.settings.software.modal.content', { software: label }).md()}
      </ConfirmationModal>

      <Stack h='100%'>
        <Select
          label={t('pages.server.settings.software.label', {})}
          description={t('pages.server.settings.software.description', {})}
          data={software.variants.map((choice) => ({ value: choice.value, label: choice.label }))}
          value={variant}
          onChange={setVariant}
          allowDeselect={false}
        />

        <Group mt='auto'>
          <Button
            variant='default'
            disabled={!variant || variant === software.current}
            onClick={() => setConfirming(true)}
          >
            {t('pages.server.settings.software.button', {})}
          </Button>
        </Group>
      </Stack>
    </TitleCard>
  );
}
