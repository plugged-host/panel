// plugged.host: the Pelican panel's "Version and software" section (ServerSettings::versionSections):
// the server's software and Minecraft version, applied by a reinstall (docs/features/server-settings.md)
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import getSoftwareVersions from '@/api/server/settings/getSoftwareVersions.ts';
import switchServerSoftware from '@/api/server/settings/switchServerSoftware.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Select from '@/elements/input/Select.tsx';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { sectionAnchor, useSoftwareOptions } from './useSettingsSections.ts';

const LATEST = 'latest';
// Filament's helper text sits under the field
const HELPER_UNDER: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

export default function SoftwareContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const base = 'pages.server.settings.software';

  const { resource, usable } = useSoftwareOptions();
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

  if (!software || !usable) {
    // the section stays while its choices load again (the page waits for the first answer itself)
    return resource.loading ? (
      <TitleCard
        {...sectionAnchor('version')}
        title={t(`${base}.title`, {})}
        description={t(`${base}.description`, {})}
      >
        <Spinner.Centered />
      </TitleCard>
    ) : null;
  }

  const variantChanged = choosesSoftware && !!variant && variant !== software.current;
  const versionChanged = choosesVersion && !!version && version !== software.version;
  const variantDescription = software.variants.find((choice) => choice.value === variant)?.description;

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
    <TitleCard
      {...sectionAnchor('version')}
      title={t(`${base}.title`, {})}
      description={t(`${base}.description`, {})}
      footer={
        <Button variant='default' disabled={!variantChanged && !versionChanged} onClick={() => setConfirming(true)}>
          {t(`${base}.button`, {})}
        </Button>
      }
    >
      <ConfirmationModal
        opened={confirming}
        onClose={() => setConfirming(false)}
        title={t(`${base}.modal.title`, {})}
        confirm={t(`${base}.modal.button`, {})}
        onConfirmed={doSwitch}
      >
        <Text size='sm' c='dimmed'>
          {t(`${base}.modal.content`, {})}
        </Text>
      </ConfirmationModal>

      {/* the section's two columns from lg, Filament's `columns(2)` */}
      <div className='grid grid-cols-1 gap-6 lg:grid-cols-2'>
        {choosesSoftware && (
          <Select
            withAsterisk
            label={t(`${base}.label`, {})}
            description={variantDescription ?? undefined}
            inputWrapperOrder={HELPER_UNDER}
            data={software.variants.map((choice) => ({ value: choice.value, label: choice.label }))}
            value={variant}
            onChange={setVariant}
            allowDeselect={false}
          />
        )}
        {choosesVersion && (
          <Select
            withAsterisk
            label={t(`${base}.versionLabel`, {})}
            description={versions.isError ? t(`${base}.versionsFailed`, {}) : t(`${base}.versionDescription`, {})}
            inputWrapperOrder={HELPER_UNDER}
            data={versionData}
            value={version}
            onChange={setVersion}
            allowDeselect={false}
            searchable
          />
        )}
      </div>
    </TitleCard>
  );
}
