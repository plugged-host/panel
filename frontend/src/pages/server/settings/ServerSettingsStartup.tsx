// plugged.host: the server's Startup page, a page of its own in the sidebar, with the Pelican panel's
// "Startup command" section (ServerSettings::startupSections, settings/startup-command.blade.php):
// the command in a well ("Enable Preview" fills its values in), the startup command and Docker image
// side by side, then the server variables, each saved on its own. Replaces upstream's
// pages/server/startup/ServerStartup.tsx on /startup (docs/features/server-settings.md)
import { useDebouncedValue } from '@mantine/hooks';
import classNames from 'classnames';
import { ReactNode, useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { httpErrorToHuman } from '@/api/axios.ts';
import getStartupPreview from '@/api/server/startup/getStartupPreview.ts';
import getVariables from '@/api/server/startup/getVariables.ts';
import updateCommand from '@/api/server/startup/updateCommand.ts';
import updateDockerImage from '@/api/server/startup/updateDockerImage.ts';
import updateVariables from '@/api/server/startup/updateVariables.ts';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ErrorState from '@/elements/feedback/ErrorState.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Select from '@/elements/input/Select.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { CopyActionIcon } from '@/elements/SettingRows.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import CustomStartupCommand from '@/pages/server/startup/CustomStartupCommand.tsx';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { trackActivity } from '@/stores/activity.ts';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';
import StartupVariableField from './StartupVariableField.tsx';

const base = 'pages.server.settings.startup';

// Filament's hint action: a link at the end of the field's label line, outside the label itself.
function WithHintAction({
  action,
  className,
  children,
}: {
  action: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div className={classNames('theme-field-action-wrap', className)}>
      {children}
      {action}
    </div>
  );
}

function StartupCommand() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const settings = useGlobalStore((state) => state.settings);
  const server = useServerStore((state) => state.server);
  const variables = useServerStore((state) => state.variables);
  const updateServer = useServerStore((state) => state.updateServer);
  const canModifyStartupCommand = useServerCan('startup.command');
  const canModifyDockerImage = useServerCan('startup.docker-image');
  const canCustomCommand = useServerCan('startup.custom-command');

  const presets = Object.entries(server.egg.startupCommands);
  const isPreset = presets.some(([, value]) => value === server.startup);
  const allowCustom = !!server.eggConfiguration?.startupAllowCustomCommand;
  // without the egg's leave, a command of one's own is checked before it is applied (the panel's
  // rules, then neo where it is on; docs/features/startup-commands.md)
  const checksCustom = canCustomCommand && !(allowCustom && canModifyStartupCommand);

  // "Custom" in the list opens the command for editing where the egg allows commands of its own, or
  // where they are checked
  const [custom, setCustom] = useState(!isPreset);
  const [command, setCommand] = useState(server.startup);

  useEffect(() => setCommand(server.startup), [server.startup]);

  const editsCustom = custom && allowCustom && canModifyStartupCommand;
  const editsChecked = custom && checksCustom;
  const editing = editsCustom || editsChecked;
  // the well shows a checked command while it is written; its preview follows once typing pauses
  const draft = editsChecked ? command : server.startup;
  const [previewed] = useDebouncedValue(draft, 400);

  // Pelican's PreviewStartupAction: the well shows the command with its values filled in, by the
  // panel, which hides the variables the viewer may not read. Fetched again when any of them changes.
  const [previewing, setPreviewing] = useState(false);
  const preview = useResource({
    queryKey: queryKeys.server(server.uuid).settings.startupPreview({
      startup: previewed,
      memory: server.limits.memory,
      allocation: server.allocation?.uuid ?? null,
      values: variables.map((variable) => variable.value),
    }),
    queryFn: () => getStartupPreview(server.uuid, previewed === server.startup ? undefined : previewed),
    enabled: previewing,
    keepPrevious: true,
  });
  const shown = previewing && preview.data !== undefined ? preview.data : draft;

  const previewAction = (
    <button type='button' className='theme-eco-link theme-field-action' onClick={() => setPreviewing((on) => !on)}>
      {t(previewing ? `${base}.disablePreview` : `${base}.enablePreview`, {})}
    </button>
  );

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

  return (
    <>
      <div className='theme-setting-cmd'>
        <span className='theme-setting-cmd-text font-mono'>{shown}</span>
        <CopyActionIcon content={shown} label={t(`${base}.copyCommand`, {})} />
      </div>

      <div className='grid grid-cols-1 gap-6 mt-6 md:grid-cols-2'>
        {presets.length > 0 ? (
          <WithHintAction action={previewAction}>
            <Select
              label={t(`${base}.command`, {})}
              data={[
                ...(allowCustom || checksCustom || !isPreset ? [{ label: t('common.custom', {}), value: '' }] : []),
                ...presets.map(([label, value]) => ({ value, label, disabled: !canModifyStartupCommand })),
              ]}
              value={custom ? '' : server.startup}
              disabled={!(canModifyStartupCommand || checksCustom) || (!allowCustom && !checksCustom && !isPreset)}
              onChange={(value) => {
                if (!value) {
                  setCustom(true);
                  return;
                }

                setCustom(false);
                saveCommand(value);
              }}
            />
          </WithHintAction>
        ) : (
          !editing && (
            <WithHintAction action={previewAction}>
              <TextInput label={t(`${base}.command`, {})} value={t(`${base}.custom`, {})} readOnly />
            </WithHintAction>
          )
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

      {/* without a list of commands, this field carries the preview action */}
      {editsCustom && (
        <WithHintAction className='mt-6' action={presets.length === 0 ? previewAction : null}>
          <TextArea
            label={t(`${base}.custom`, {})}
            value={command}
            autosize
            onChange={(e) => setCommand(e.currentTarget.value)}
            onBlur={() => saveCommand(command)}
          />
        </WithHintAction>
      )}
      {editsChecked && (
        <CustomStartupCommand
          command={command}
          onChange={setCommand}
          action={presets.length === 0 ? previewAction : null}
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
        <div className='theme-startup-variables grid grid-cols-1 gap-x-6 gap-y-8 lg:grid-cols-2'>
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
    <ServerContentContainer
      title={t('pages.server.startup.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.server.startup.container}
    >
      <TitleCard title={t(`${base}.title`, {})}>
        <StartupCommand />
        <StartupVariables />
      </TitleCard>
    </ServerContentContainer>
  );
}
