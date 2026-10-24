// plugged.host: a new world in place of the loaded one (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createWorld from '@/api/server/minecraft/createWorld.ts';
import Button from '@/elements/buttons/Button.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

export const WORLD_NAME = /^[A-Za-z0-9][A-Za-z0-9 _.-]{0,63}$/;

type Props = ModalProps & {
  worlds: z.infer<typeof minecraftWorldsSchema>;
};

export default function NewWorldModal({ worlds, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.worlds';

  const [generation, setGeneration] = useState(worlds.generation);
  const [datapacks, setDatapacks] = useState(worlds.generation.datapacks.join(', '));
  const [keep, setKeep] = useState(true);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!props.opened) return;

    setGeneration(worlds.generation);
    setDatapacks(worlds.generation.datapacks.join(', '));
    setKeep(true);
  }, [props.opened]);

  const typeLabels: Record<string, string> = {
    'minecraft:normal': t(`${base}.types.normal`, {}),
    'minecraft:flat': t(`${base}.types.flat`, {}),
    'minecraft:large_biomes': t(`${base}.types.largeBiomes`, {}),
    'minecraft:amplified': t(`${base}.types.amplified`, {}),
    'minecraft:single_biome_surface': t(`${base}.types.singleBiome`, {}),
  };

  const name = generation.name.trim();
  const nameValid = WORLD_NAME.test(name) && !name.includes('..');
  const nameTaken =
    name !== worlds.generation.name && worlds.worlds.some((world) => world.name.toLowerCase() === name.toLowerCase());
  const usesGenerator = ['minecraft:flat', 'minecraft:single_biome_surface'].includes(generation.levelType);

  const submit = () => {
    setLoading(true);

    createWorld(
      server.uuid,
      {
        ...generation,
        name,
        datapacks: datapacks
          .split(',')
          .map((pack) => pack.trim())
          .filter(Boolean),
      },
      keep,
    )
      .then((replaced) => {
        addToast(
          replaced.kept
            ? `${t(`${base}.toast.created`, {})} ${t(`${base}.toast.kept`, { name: replaced.kept })}`
            : t(`${base}.toast.created`, {}),
          'success',
        );
        props.onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setLoading(false);
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });
      });
  };

  return (
    <Modal title={t(`${base}.modal.new.title`, {})} {...props}>
      <Stack>
        <Text size='sm'>{t(`${base}.modal.new.content`, {})}</Text>

        <TextInput
          withAsterisk
          label={t(`${base}.form.name`, {})}
          description='level-name'
          maxLength={64}
          value={generation.name}
          onChange={(e) => setGeneration({ ...generation, name: e.currentTarget.value })}
          error={
            name && !nameValid
              ? t(`${base}.form.nameInvalid`, {})
              : nameTaken
                ? t(`${base}.form.nameTaken`, {})
                : undefined
          }
        />
        <TextInput
          label={t(`${base}.form.seed`, {})}
          description='level-seed'
          placeholder={t(`${base}.form.seedRandom`, {})}
          maxLength={64}
          value={generation.seed}
          onChange={(e) => setGeneration({ ...generation, seed: e.currentTarget.value })}
        />
        <Select
          label={t(`${base}.form.type`, {})}
          description='level-type'
          data={worlds.levelTypes.map((type) => ({ value: type, label: typeLabels[type] ?? type }))}
          value={generation.levelType}
          onChange={(value) => value && setGeneration({ ...generation, levelType: value })}
        />
        {usesGenerator && (
          <TextInput
            label={t(`${base}.form.generator`, {})}
            description='generator-settings'
            maxLength={2000}
            value={generation.generator}
            onChange={(e) => setGeneration({ ...generation, generator: e.currentTarget.value })}
          />
        )}
        <TextInput
          label={t(`${base}.form.datapacks`, {})}
          description={t(`${base}.form.datapacksDescription`, {})}
          value={datapacks}
          onChange={(e) => setDatapacks(e.currentTarget.value)}
        />
        <Switch
          label={t(`${base}.form.structures`, {})}
          checked={generation.structures}
          onChange={(e) => setGeneration({ ...generation, structures: e.currentTarget.checked })}
        />
        <Checkbox
          label={t(`${base}.form.keep`, {})}
          description={
            name === worlds.generation.name ? t(`${base}.form.keepRenamed`, {}) : t(`${base}.form.keepAsIs`, {})
          }
          checked={keep}
          onChange={(e) => setKeep(e.currentTarget.checked)}
        />

        <ModalFooter>
          <Button onClick={submit} loading={loading} disabled={!nameValid || nameTaken}>
            {t(`${base}.button.create`, {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
