// plugged.host: installing a chosen version of a plugin or mod (docs/features/minecraft.md)
import { ModalProps } from '@mantine/core';
import { useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getContentVersions from '@/api/server/minecraft/getContentVersions.ts';
import installContent from '@/api/server/minecraft/installContent.ts';
import Button from '@/elements/buttons/Button.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
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
import { minecraftVersionLabel } from './labels.ts';
import { useInstalledToast } from './useInstalledToast.ts';

type Props = ModalProps & {
  project: z.infer<typeof minecraftProjectSchema>;
  gameVersion: string | null;
  onInstalled: () => void;
};

export default function InstallVersionModal({ project, gameVersion, onInstalled, ...props }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const installedToast = useInstalledToast();
  const base = 'pages.server.minecraft.content.browse.modal';

  const [versionId, setVersionId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const versions = useResource({
    queryKey: queryKeys.server(server.uuid).minecraft.versions(project.source, project.id),
    queryFn: () => getContentVersions(server.uuid, project.source, project.id, project.kind),
    enabled: props.opened,
  });

  const list = versions.data?.versions ?? [];
  const selected = versionId ?? list[0]?.id ?? null;
  const hint = [
    project.source === 'spigotmc' ? t(`${base}.spigotLatestOnly`, {}) : null,
    versions.data?.loose && gameVersion ? t(`${base}.loose`, { version: gameVersion }) : null,
  ]
    .filter(Boolean)
    .join(' ');

  const install = () => {
    if (!selected) return;
    setLoading(true);

    installContent(server.uuid, { source: project.source, project: project.id, version: selected })
      .then((result) => {
        installedToast(result);
        onInstalled();
        props.onClose();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setLoading(false));
  };

  return (
    <Modal title={t(`${base}.title`, { name: project.title })} {...props}>
      <Stack>
        {project.clientOnly && (
          <Text size='sm' c='dimmed'>
            {t(`${base}.clientOnly`, {})}
          </Text>
        )}

        {versions.loading && !versions.data ? (
          <Spinner.Centered />
        ) : list.length === 0 ? (
          <Text size='sm' c='dimmed'>
            {t(`${base}.noVersions`, {})}
          </Text>
        ) : (
          <Select
            label={t('common.table.columns.version', {})}
            description={hint || undefined}
            inputWrapperOrder={['label', 'input', 'description', 'error']}
            data={list.map((version) => ({ value: version.id, label: minecraftVersionLabel(version) }))}
            value={selected}
            onChange={setVersionId}
            searchable
          />
        )}

        <ModalFooter>
          <Button onClick={install} loading={loading} disabled={!selected}>
            {t('common.button.install', {})}
          </Button>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.cancel', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
