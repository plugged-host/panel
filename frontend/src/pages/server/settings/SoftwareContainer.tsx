// plugged.host: switch the server's software and Minecraft version, which reinstalls it (docs/features/server-settings.md)
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import getServerSoftware from '@/api/server/settings/getServerSoftware.ts';
import getSoftwareVersions from '@/api/server/settings/getSoftwareVersions.ts';
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

const LATEST = 'latest';

export default function SoftwareContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const base = 'pages.server.settings.software';

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.software(),
    queryFn: () => getServerSoftware(server.uuid),
    silent: true,
  });
  const software = resource.data;
  const choosesSoftware = !!software && software.variants.length >= 2;
  const choosesVersion = !!software && software.version !== null;

  const [variant, setVariant] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!software) return;
    setVariant(software.current);
    setVersion(software.version);
  }, [software]);

  const versionVariant = choosesSoftware ? variant : null;
  const versions = useQuery({
    queryKey: queryKeys.server(server.uuid).settings.softwareVersions(versionVariant),
    queryFn: () => getSoftwareVersions(server.uuid, versionVariant),
    enabled: choosesVersion,
    retry: false,
  });

  if (!software || (!choosesSoftware && !choosesVersion)) {
    return null;
  }

  const variantChanged = choosesSoftware && !!variant && variant !== software.current;
  const versionChanged = choosesVersion && !!version && version !== software.version;
  const label = software.variants.find((choice) => choice.value === variant)?.label ?? '';
  const versionLabel = version === LATEST ? t(`${base}.versionLatest`, {}) : (version ?? '');

  const versionData = [
    { value: LATEST, label: t(`${base}.versionLatest`, {}) },
    ...(versions.data ?? []).map((value) => ({ value, label: value })),
  ];
  // a version the list no longer has (or does not have yet) stays choosable
  if (software.version && !versionData.some((choice) => choice.value === software.version)) {
    versionData.push({ value: software.version, label: software.version });
  }

  const doSwitch = async () => {
    await switchServerSoftware(server.uuid, {
      variant: variantChanged ? variant : null,
      version: versionChanged ? version : null,
    });
    addToast(t(`${base}.toast.switching`, {}), 'success');
    setConfirming(false);
    resource.invalidate();
    navigate(`/server/${server.uuidShort}`);
    updateServer({ status: 'installing' });
  };

  return (
    <TitleCard title={t(`${base}.title`, {})} className='h-full order-55'>
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${base}.modal.title`, {})}
        confirm={t(`${base}.modal.button`, {})}
        onConfirmed={doSwitch}
      >
        {(variantChanged && versionChanged
          ? t(`${base}.modal.contentBoth`, { software: label, version: versionLabel })
          : versionChanged
            ? t(`${base}.modal.contentVersion`, { version: versionLabel })
            : t(`${base}.modal.content`, { software: label })
        ).md()}
      </ConfirmationModal>

      <Stack h='100%'>
        {choosesSoftware && (
          <Select
            label={t(`${base}.label`, {})}
            description={t(`${base}.description`, {})}
            data={software.variants.map((choice) => ({ value: choice.value, label: choice.label }))}
            value={variant}
            onChange={setVariant}
            allowDeselect={false}
          />
        )}
        {choosesVersion && (
          <Select
            label={t(`${base}.versionLabel`, {})}
            description={versions.isError ? t(`${base}.versionsFailed`, {}) : t(`${base}.versionDescription`, {})}
            data={versionData}
            value={version}
            onChange={setVersion}
            allowDeselect={false}
            searchable
          />
        )}

        <Group mt='auto'>
          <Button variant='default' disabled={!variantChanged && !versionChanged} onClick={() => setConfirming(true)}>
            {t(`${base}.button`, {})}
          </Button>
        </Group>
      </Stack>
    </TitleCard>
  );
}
