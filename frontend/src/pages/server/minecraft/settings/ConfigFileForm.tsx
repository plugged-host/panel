// plugged.host: one settings file as a form; only the changed values are sent (docs/features/minecraft.md)
import { useEffect, useMemo, useState } from 'react';
import { createSearchParams, useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import updateGameSettings from '@/api/server/minecraft/updateGameSettings.ts';
import Button from '@/elements/buttons/Button.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import Title from '@/elements/typography/Title.tsx';
import { minecraftConfigFileSchema } from '@/lib/schemas/server/minecraft.ts';
import { useServerCan } from '@/plugins/usePermissions.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import ConfigField, { ConfigValue, humanize } from './ConfigField.tsx';

type ConfigFile = z.infer<typeof minecraftConfigFileSchema>;

const keyOf = (path: string[]) => path.join('\u0000');

const initialValues = (file: ConfigFile) =>
  Object.fromEntries(file.fields.map((field) => [keyOf(field.path), field.value]));

interface Props {
  file: ConfigFile;
  search: string;
  onSaved: () => void;
}

export default function ConfigFileForm({ file, search, onSaved }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const navigate = useNavigate();
  const server = useServerStore((state) => state.server);
  const canEditText = useServerCan(['files.read-content', 'files.update'], false);
  const base = 'pages.server.minecraft.settings';

  const [values, setValues] = useState<Record<string, ConfigValue>>(() => initialValues(file));
  const [saving, setSaving] = useState(false);

  useEffect(() => setValues(initialValues(file)), [file]);

  const changed = file.fields.filter((field) => values[keyOf(field.path)] !== field.value);

  // YAML: one group per top-level section; values at the top come first, untitled
  const groups = useMemo(() => {
    const query = search.trim().toLowerCase();
    const map = new Map<string, ConfigFile['fields']>();

    for (const field of file.fields) {
      if (query && !field.path.join('.').toLowerCase().includes(query)) continue;

      const group = file.format === 'yaml' && field.path.length > 1 ? field.path[0] : '';
      map.set(group, [...(map.get(group) ?? []), field]);
    }

    return [...map.entries()].sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : 0));
  }, [file, search]);

  const save = () => {
    setSaving(true);

    updateGameSettings(
      server.uuid,
      file.id,
      changed.map((field) => ({ path: field.path, value: values[keyOf(field.path)] ?? null })),
    )
      .then((count) => {
        addToast(t(`${base}.toast.saved`, { count, file: file.path }), 'success');
        onSaved();
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

  const slash = file.path.lastIndexOf('/');
  const editorUrl = `/server/${server.uuidShort}/files/edit?${createSearchParams({
    directory: slash === -1 ? '/' : `/${file.path.slice(0, slash)}`,
    file: file.path.slice(slash + 1),
  })}`;

  return (
    <Stack gap='lg'>
      <Text size='sm' c='dimmed'>
        {file.format === 'properties' ? t(`${base}.propertiesNote`, {}) : t(`${base}.yamlNote`, {})}
      </Text>

      {groups.length === 0 && (
        <Text size='sm' c='dimmed'>
          {t(`${base}.noMatch`, {})}
        </Text>
      )}

      {groups.map(([group, fields]) => (
        <Stack key={group} gap='sm'>
          {group && (
            <Title order={3} fz='md' fw={600}>
              {humanize(group)}
            </Title>
          )}
          <div className='grid gap-4 md:grid-cols-2'>
            {fields.map((field) => (
              <ConfigField
                key={keyOf(field.path)}
                field={field}
                value={values[keyOf(field.path)] ?? null}
                disabled={saving}
                onChange={(value) => setValues((values) => ({ ...values, [keyOf(field.path)]: value }))}
              />
            ))}
          </div>
        </Stack>
      ))}

      <Group>
        <Button onClick={save} loading={saving} disabled={changed.length === 0}>
          {t('common.button.save', {})}
        </Button>
        {canEditText && (
          <Button variant='default' onClick={() => navigate(editorUrl)}>
            {t(`${base}.button.editText`, {})}
          </Button>
        )}
      </Group>
    </Stack>
  );
}
