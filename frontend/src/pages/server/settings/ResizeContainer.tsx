// plugged.host: resize within the owner's allowance (docs/features/server-settings.md)
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerResize from '@/api/server/settings/getServerResize.ts';
import resizeServer from '@/api/server/settings/resizeServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import SizeInput from '@/elements/input/SizeInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import Text from '@/elements/typography/Text.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSettingsResizeSchema } from '@/lib/schemas/server/settings.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { useServerStore } from '@/stores/server.ts';

type Values = z.infer<typeof serverSettingsResizeSchema>;
type Field = keyof Values;

export default function ResizeContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const { user } = useAuth();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const allowResizing = useGlobalStore((state) => state.settings.games.allowResizing);

  const resource = useResource({
    queryKey: queryKeys.server(server.uuid).settings.resize(),
    queryFn: () => getServerResize(server.uuid),
    enabled: allowResizing || !!user?.admin,
    silent: true,
  });
  const options = resource.data;

  const [values, setValues] = useState<Values>({
    cpu: server.limits.cpu,
    memory: server.limits.memory,
    disk: server.limits.disk,
  });
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (options) setValues(options.current);
  }, [options]);

  if (!options || (!options.enabled && !user?.admin)) {
    return null;
  }

  const error = (field: Field): string | undefined => {
    const value = values[field];
    const max = options.max[field];

    if (value < 1 && !(value === 0 && options.current[field] === 0)) {
      return t('pages.server.settings.resize.atLeastOne', {});
    }
    if (max !== null && value > max && value > options.current[field]) {
      return t('pages.server.settings.resize.overMax', {});
    }

    return undefined;
  };

  const upTo = (field: Field, format: (value: number) => string) => {
    const max = options.max[field];

    return max === null
      ? t('pages.server.settings.resize.noLimit', {})
      : t('pages.server.settings.resize.upTo', { max: format(max) });
  };

  const changed = (['cpu', 'memory', 'disk'] as Field[]).some((field) => values[field] !== options.current[field]);
  const valid = (['cpu', 'memory', 'disk'] as Field[]).every((field) => !error(field));

  const doResize = () => {
    setLoading(true);
    resizeServer(server.uuid, values)
      .then(() => {
        addToast(t('pages.server.settings.resize.toast.resized', {}), 'success');
        updateServer({ limits: { ...server.limits, ...values } });
        resource.invalidate();
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <TitleCard title={t('pages.server.settings.resize.title', {})} className='h-full order-15'>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          doResize();
        }}
        className='h-full'
      >
        <Stack h='100%'>
          <NumberInput
            label={t('pages.server.settings.resize.cpu', {})}
            description={upTo('cpu', (value) => `${value}%`)}
            min={0}
            value={values.cpu}
            error={error('cpu')}
            onChange={(value) => setValues((current) => ({ ...current, cpu: Number(value) || 0 }))}
          />
          <SizeInput
            label={t('common.form.memory', {})}
            description={upTo('memory', (value) => bytesToString(mbToBytes(value)))}
            mode='mb'
            min={0}
            value={values.memory}
            onChange={(value) => setValues((current) => ({ ...current, memory: value }))}
          />
          <SizeInput
            label={t('common.form.disk', {})}
            description={upTo('disk', (value) => bytesToString(mbToBytes(value)))}
            mode='mb'
            min={0}
            value={values.disk}
            onChange={(value) => setValues((current) => ({ ...current, disk: value }))}
          />
          {(error('memory') || error('disk')) && (
            <Text c='red' size='sm'>
              {error('memory') ?? error('disk')}
            </Text>
          )}

          <Group mt='auto'>
            <Button type='submit' loading={loading} disabled={!changed || !valid}>
              {t('common.button.save', {})}
            </Button>
          </Group>
        </Stack>
      </form>
    </TitleCard>
  );
}
