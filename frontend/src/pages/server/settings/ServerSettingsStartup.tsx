// plugged.host: the server's Startup page as the Pelican panel's "Startup command" section
// (ServerSettings::startupSections, settings/startup-command.blade.php): the command in a well,
// the startup command and Docker image side by side, then the server variables, each saved on its
// own. Replaces upstream's pages/server/startup/ServerStartup.tsx on /startup
// (docs/features/server-settings.md)
import { useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { httpErrorToHuman } from '@/api/axios.ts';
import getVariables from '@/api/server/startup/getVariables.ts';
import updateCommand from '@/api/server/startup/updateCommand.ts';
import updateDockerImage from '@/api/server/startup/updateDockerImage.ts';
import updateVariables from '@/api/server/startup/updateVariables.ts';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { CopyActionIcon } from '@/elements/SettingRows.tsx';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { trackActivity } from '@/stores/activity.ts';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';
import ServerSettingsShell from './ServerSettingsShell.tsx';
import StartupVariableField from './StartupVariableField.tsx';

const base = 'pages.server.settings.startup';

function StartupCommand() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const settings = useGlobalStore((state) => state.settings);
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const canModifyStartupCommand = useServerCan('startup.command');
  const canModifyDockerImage = useServerCan('startup.docker-image');

  const presets = Object.entries(server.egg.startupCommands);
  const isPreset = presets.some(([, value]) => value === server.startup);
  const allowCustom = !!server.eggConfiguration?.startupAllowCustomCommand;

  // "Custom" in the list opens the command for editing where the egg allows commands of its own
  const [custom, setCustom] = useState(!isPreset);
  const [command, setCommand] = useState(server.startup);

  useEffect(() => setCommand(server.startup), [server.startup]);

  const images = Object.entries(server.egg.dockerImages);
  const isEggImage = images.some(([, value]) => value === server.image);
  const imageChoosable = isEggImage || settings.server.allowOverwritingCustomDockerImage;

  const saveCommand = (next: string) => {
    if (!next.trim() || next === server.startup) return;

    updateCommand(server.uuid, next)
      .then(() => {
        addToast(t(`${base}.commandUpdated`, {}), 'success');
        updateServer({ startup: next });
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
        setCommand(server.startup);
      });
  };

  const saveImage = (next: string | null) => {
    if (!next || next === server.image) return;

    updateDockerImage(server.uuid, next)
      .then(() => {
        addToast(t(`${base}.imageUpdated`, {}), 'success');
        updateServer({ image: next });
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      });
  };

  const editsCustom = custom && allowCustom && canModifyStartupCommand;

  return (
    <>
      <div className='theme-setting-cmd'>
        <span className='theme-setting-cmd-text font-mono'>{server.startup}</span>
        <CopyActionIcon content={server.startup} label={t(`${base}.copyCommand`, {})} />
      </div>

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4 mt-6'>
        {presets.length > 0 ? (
          <Select
            label={t(`${base}.command`, {})}
            data={[
              ...(allowCustom || !isPreset ? [{ label: t('common.custom', {}), value: '' }] : []),
              ...presets.map(([label, value]) => ({ value, label })),
            ]}
            value={custom ? '' : server.startup}
            disabled={!canModifyStartupCommand || (!allowCustom && !isPreset)}
            onChange={(value) => {
              if (!value) {
                setCustom(true);
                return;
              }

              setCustom(false);
              saveCommand(value);
            }}
          />
        ) : (
          !editsCustom && <TextInput label={t(`${base}.command`, {})} value={t(`${base}.custom`, {})} readOnly />
        )}

        {imageChoosable ? (
          <Select
            label={t(`${base}.image`, {})}
            data={images.map(([label, value]) => ({ value, label }))}
            value={isEggImage ? server.image : null}
            placeholder={server.image}
            searchable
            disabled={!canModifyDockerImage}
            onChange={saveImage}
          />
        ) : (
          <TextInput label={t(`${base}.image`, {})} value={server.image} readOnly />
        )}
      </div>

      {editsCustom && (
        <TextArea
          mt='md'
          label={t(`${base}.custom`, {})}
          value={command}
          autosize
          onChange={(e) => setCommand(e.currentTarget.value)}
          onBlur={() => saveCommand(command)}
        />
      )}
    </>
  );
}

function StartupVariables() {
  const { t, language } = useTranslations();
  const { addToast } = useToast();
  const { server, variables, setVariables, updateVariable } = useServerStore(
    useShallow((state) => ({
      server: state.server,
      variables: state.variables,
      setVariables: state.setVariables,
      updateVariable: state.updateVariable,
    })),
  );
  const canModifyVariables = useServerCan('startup.update');

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);

  const load = () => {
    setLoading(true);
    setError(null);

    trackActivity(getVariables(server.uuid))
      .then(setVariables)
      .catch(setError)
      .finally(() => setLoading(false));
  };

  useEffect(load, []);

  const save = (envVariable: string, name: string, value: string) =>
    updateVariables(server.uuid, [{ envVariable, value }])
      .then(() => {
        addToast(t(`${base}.variableUpdated`, { variable: name }), 'success');
        updateVariable(envVariable, { value });
        return true;
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
        return false;
      });

  return (
    <>
      <h3 className='theme-eco-heading mt-6 mb-4'>{t(`${base}.variables`, {})}</h3>

      {loading ? (
        <Spinner.Centered />
      ) : error ? (
        <ErrorState error={httpErrorToHuman(error)} onRetry={load} />
      ) : variables.length === 0 ? (
        <p className='theme-setting-note'>{t('pages.server.startup.empty.description', {})}</p>
      ) : (
        <div className='grid grid-cols-1 xl:grid-cols-2 gap-x-4 gap-y-6'>
          {variables.map((variable) => (
            <StartupVariableField
              key={variable.envVariable}
              variable={variable}
              disabled={!canModifyVariables}
              onSave={(value) =>
                save(variable.envVariable, variable.nameTranslations?.[language] || variable.name, value)
              }
            />
          ))}
        </div>
      )}
    </>
  );
}

export default function ServerSettingsStartup() {
  const { t } = useTranslations();

  return (
    <ServerSettingsShell registry={window.extensionContext.extensionRegistry.pages.server.startup.container}>
      <TitleCard title={t(`${base}.title`, {})}>
        <StartupCommand />
        <StartupVariables />
      </TitleCard>
    </ServerSettingsShell>
  );
}
