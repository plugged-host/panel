// plugged.host: "Options" on the loaded world: the settings a world is generated with, saved by
// starting a new world (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import createWorld from '@/api/server/minecraft/createWorld.ts';
import Button from '@/elements/buttons/Button.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TagsInput from '@/elements/input/TagsInput.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftWorldsSchema } from '@/lib/schemas/server/minecraft.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useReplacedToast } from './useReplacedToast.ts';

const WORLD_NAME = /^[A-Za-z0-9][A-Za-z0-9 _.-]{0,63}$/;
const DATAPACK = /^[a-z0-9_.:/-]{1,64}$/;

type Props = ModalProps & {
  worlds: z.infer<typeof minecraftWorldsSchema>;
};

export default function WorldOptionsModal({ worlds, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const replacedToast = useReplacedToast();
  const queryClient = useQueryClient();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.worlds';

  const [generation, setGeneration] = useState(worlds.generation);
  const [keep, setKeep] = useState(true);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!props.opened) return;

    setGeneration(worlds.generation);
    setKeep(true);
  }, [props.opened]);

  const name = generation.name.trim();
  const nameValid = WORLD_NAME.test(name) && !name.includes('..');
  const nameTaken =
    name !== worlds.generation.name && worlds.worlds.some((world) => world.name.toLowerCase() === name.toLowerCase());
  const badPacks = generation.datapacks.filter((pack) => !DATAPACK.test(pack));

  const submit = () => {
    setLoading(true);

    createWorld(server.uuid, { ...generation, name }, keep)
      .then((replaced) => {
        replacedToast(t(`${base}.toast.recreated`, { world: name }), replaced.kept);
        props.onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => {
        setLoading(false);
        queryClient.invalidateQueries({ queryKey: queryKeys.server(server.uuid).minecraft.worlds() });
      });
  };

  return (
    <Modal title={t(`${base}.modal.options.title`, {})} {...props}>
      <Stack>
        <Text size='sm' c='red'>
          {t(`${base}.deletesCurrent`, {})}
        </Text>

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
        <TextInput
          label={t(`${base}.form.generator`, {})}
          description='generator-settings'
          maxLength={2000}
          value={generation.generator}
          onChange={(e) => setGeneration({ ...generation, generator: e.currentTarget.value })}
        />
        <Select
          withAsterisk
          label={t(`${base}.form.type`, {})}
          description='level-type'
          data={worlds.levelTypes}
          value={generation.levelType}
          onChange={(value) => value && setGeneration({ ...generation, levelType: value })}
        />
        <Switch
          label={t(`${base}.form.structures`, {})}
          description='generate-structures'
          checked={generation.structures}
          onChange={(e) => setGeneration({ ...generation, structures: e.currentTarget.checked })}
        />
        <TagsInput
          label={t(`${base}.form.datapacks`, {})}
          description='initial-enabled-packs'
          placeholder={t(`${base}.form.datapacksPlaceholder`, {})}
          allowReordering={false}
          value={generation.datapacks}
          onChange={(datapacks) => setGeneration({ ...generation, datapacks })}
          invalidTags={badPacks}
        />
        <Switch
          label={t(`${base}.form.keep`, {})}
          description={
            name === worlds.generation.name ? t(`${base}.form.keepRenamed`, {}) : t(`${base}.form.keepAsIs`, {})
          }
          checked={keep}
          onChange={(e) => setKeep(e.currentTarget.checked)}
        />

        <ModalFooter>
          <Button onClick={submit} loading={loading} disabled={!nameValid || nameTaken || badPacks.length > 0}>
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
