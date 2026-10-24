// plugged.host: choosing a modpack's version and installing it (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getModpackVersions from '@/api/server/minecraft/getModpackVersions.ts';
import installModpack from '@/api/server/minecraft/installModpack.ts';
import Button from '@/elements/buttons/Button.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Checkbox from '@/elements/input/Checkbox.tsx';
import Select from '@/elements/input/Select.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { minecraftProjectSchema } from '@/lib/schemas/server/minecraft.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

type Props = ModalProps & {
  project: z.infer<typeof minecraftProjectSchema>;
  onStarted: () => void;
};

export default function ModpackModal({ project, onStarted, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const base = 'pages.server.minecraft.modpacks';

  const [versionId, setVersionId] = useState<string | null>(null);
  const [clean, setClean] = useState(true);
  const [loading, setLoading] = useState(false);

  const versions = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.modpackVersions(project.source, project.id),
    queryFn: () => getModpackVersions(server.uuid, project.source, project.id),
    enabled: props.opened,
  });

  const list = versions.data ?? [];
  const selected = list.find((version) => version.id === versionId) ?? list[0] ?? null;

  const install = () => {
    if (!selected) return;
    setLoading(true);

    installModpack(server.uuid, { source: project.source, project: project.id, version: selected.id, clean })
      .then((started) => {
        addToast(t(`${base}.toast.started`, { name: started.name }), 'success');
        onStarted();
        props.onClose();
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${base}.modal.title`, { name: project.title })} {...props}>
      <Stack>
        {versions.loading && !versions.data ? (
          <Spinner.Centered />
        ) : list.length === 0 ? (
          <Text size='sm' c='dimmed'>
            {t(`${base}.modal.noVersions`, {})}
          </Text>
        ) : (
          <Select
            label={t('common.table.columns.version', {})}
            description={
              selected?.gameVersions.length
                ? t(`${base}.modal.versionFor`, {
                    minecraft: selected.gameVersions.join(', '),
                    loaders: selected.loaders.join(', '),
                  })
                : undefined
            }
            data={list.map((version) => ({
              value: version.id,
              label: version.channel === 'release' ? version.number : `${version.number} (${version.channel})`,
            }))}
            value={selected?.id ?? null}
            onChange={setVersionId}
            searchable
          />
        )}

        <Checkbox
          label={t(`${base}.modal.clean`, {})}
          description={t(`${base}.modal.cleanDescription`, {})}
          checked={clean}
          onChange={(e) => setClean(e.currentTarget.checked)}
        />

        <Text size='sm' c='dimmed'>
          {t(`${base}.modal.content`, {})}
        </Text>

        <ModalFooter>
          <Button onClick={install} loading={loading} disabled={!selected}>
            {t(`${base}.button.install`, {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
