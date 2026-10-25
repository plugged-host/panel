// plugged.host: "Generate" on the loaded world: the same world name started over, with a new random
// seed or the same one (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createWorld from '@/api/server/minecraft/createWorld.ts';
import Button from '@/elements/buttons/Button.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useReplacedToast } from './useReplacedToast.ts';

type Props = ModalProps & {
  worlds: z.infer<typeof minecraftWorldsSchema>;
};

export default function GenerateWorldModal({ worlds, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const replacedToast = useReplacedToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.worlds';
  const world = worlds.generation.name;

  const [newSeed, setNewSeed] = useState(true);
  const [keep, setKeep] = useState(true);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!props.opened) return;

    setNewSeed(true);
    setKeep(true);
  }, [props.opened]);

  const submit = () => {
    setLoading(true);

    // The same name: the loaded world is kept under a new name or deleted, and the next start
    // generates it again from server.properties.
    createWorld(server.uuid, { ...worlds.generation, seed: newSeed ? '' : worlds.generation.seed }, keep)
      .then((replaced) => {
        replacedToast(t(`${base}.toast.recreated`, { world }), replaced.kept);
        props.onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setLoading(false);
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });
      });
  };

  return (
    <Modal title={t(`${base}.modal.generate.title`, { world })} {...props}>
      <Stack>
        <Text size='sm' c='red'>
          {t(`${base}.deletesCurrent`, {})}
        </Text>

        <Switch
          label={t(`${base}.form.newSeed`, {})}
          checked={newSeed}
          onChange={(e) => setNewSeed(e.currentTarget.checked)}
        />
        <Switch
          label={t(`${base}.form.keep`, {})}
          description={t(`${base}.form.keepRenamed`, {})}
          checked={keep}
          onChange={(e) => setKeep(e.currentTarget.checked)}
        />

        <ModalFooter>
          <Button onClick={submit} loading={loading}>
            {t(`${base}.button.generate`, {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
