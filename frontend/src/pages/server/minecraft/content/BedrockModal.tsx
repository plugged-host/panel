// plugged.host: letting Bedrock players join through Geyser + Floodgate (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import installBedrock from '@/api/server/minecraft/installBedrock.ts';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftBedrockSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Props = Omit<ModalProps, 'children' | 'title'> & {
  status: z.infer<typeof minecraftBedrockSchema>;
  // the header button's label, which is also the heading and the confirm button (Filament's confirmation)
  label: string;
};

export default function BedrockModal({ status, label, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.bedrock';

  const install = () =>
    installBedrock(server.uuid)
      .then((installed) => {
        addToast(
          installed.updated
            ? t(`${base}.toast.updated`, { address: `${installed.address}:${installed.port}` })
            : t(`${base}.toast.installed`, { address: `${installed.address}:${installed.port}` }),
          'success',
        );
        if (!installed.floodgate) {
          addToast(t(`${base}.toast.noFloodgate`, {}), 'warning');
        }
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.bedrock() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.installed() });
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).network.all() });
        props.onClose();
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      });

  return (
    <ConfirmationModal title={label} confirm={label} confirmColor='gray' onConfirmed={install} {...props}>
      <Stack gap='sm'>
        <Text size='sm'>{t(`${base}.content`, {})}</Text>

        {status.address && status.port && (
          <Stack gap={4}>
            <Text size='sm' c='dimmed'>
              {t(`${base}.join`, {})}
            </Text>
            <CopyOnClick content={`${status.address}:${status.port}`}>
              <Code>{`${status.address}:${status.port}`}</Code>
            </CopyOnClick>
          </Stack>
        )}
        {status.running && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.restart`, {})}
          </Text>
        )}
      </Stack>
    </ConfirmationModal>
  );
}
