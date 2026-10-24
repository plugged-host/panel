// plugged.host: letting Bedrock players join through Geyser + Floodgate (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import getBedrock from '@/api/server/minecraft/getBedrock.ts';
import installBedrock from '@/api/server/minecraft/installBedrock.ts';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export default function BedrockModal(props: ModalProps) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.bedrock';

  const [loading, setLoading] = useState(false);

  const bedrock = useQuery({
    queryKey: queryKeys.server(server.uuid).minecraft.bedrock(),
    queryFn: () => getBedrock(server.uuid),
    enabled: props.opened,
    retry: false,
  });
  const status = bedrock.data;

  const install = () => {
    setLoading(true);

    installBedrock(server.uuid)
      .then((installed) => {
        addToast(
          t(installed.updated ? `${base}.toast.updated` : `${base}.toast.installed`, {
            address: `${installed.address}:${installed.port}`,
          }),
          'success',
        );
        if (!installed.floodgate) {
          addToast(t(`${base}.toast.noFloodgate`, {}), 'warning');
        }
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.bedrock() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).network.all() });
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${base}.title`, {})} {...props}>
      <Stack>
        <Text size='sm'>{t(`${base}.content`, {})}</Text>

        {bedrock.isLoading && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.loading`, {})}
          </Text>
        )}
        {bedrock.error && (
          <Text size='sm' c='red'>
            {httpErrorToHuman(bedrock.error)}
          </Text>
        )}

        {status && !status.available && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.unavailable`, {})}
          </Text>
        )}

        {status?.available && status.installed && (
          <Text size='sm'>{t(`${base}.installed`, { file: status.installed })}</Text>
        )}
        {status?.available && status.address && status.port && (
          <Stack gap={4}>
            <Text size='sm' c='dimmed'>
              {t(`${base}.join`, {})}
            </Text>
            <CopyOnClick content={`${status.address}:${status.port}`}>
              <Code>{`${status.address}:${status.port}`}</Code>
            </CopyOnClick>
          </Stack>
        )}
        {status?.available && status.running && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.restart`, {})}
          </Text>
        )}

        <ModalFooter>
          {status?.available && (
            <Button onClick={install} loading={loading}>
              {t(status.installed ? `${base}.button.update` : `${base}.button.install`, {})}
            </Button>
          )}
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
