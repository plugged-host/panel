// plugged.host: Settings' "Resources": a plan, or CPU, memory and disk set by hand under "Advanced", within
// the owner's allowance, with one Save. Who may not resize sees the server's limits as rows in its place
// (docs/features/server-settings.md#resources)
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getServerUsage from '@/api/server/settings/getServerUsage.ts';
import resizeServer from '@/api/server/settings/resizeServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import Choice from '@/elements/input/Choice.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import { SettingRow, SettingRows } from '@/elements/SettingRows.tsx';
import { bytesToString, mbToBytes } from '@/lib/format/size.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { serverSettingsResizeOptionsSchema, serverSettingsResizeSchema } from '@/lib/schemas/server/settings.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import SettingsSection, { HELPER_UNDER } from './SettingsSection.tsx';
import { useResizeOptions } from './useSettingsSections.ts';

type Options = z.infer<typeof serverSettingsResizeOptionsSchema>;
type Values = z.infer<typeof serverSettingsResizeSchema>;
type Field = keyof Values;

const base = 'pages.server.settings.resize';
const general = 'pages.server.settings.general';
const FIELDS: Field[] = ['cpu', 'memory', 'disk'];

function Resize({ options, onSaved }: { options: Options; onSaved: () => void }) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);

  const [values, setValues] = useState<Values>(options.current);
  const [plan, setPlan] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [loading, setLoading] = useState(false);

  // the plan the server has now is picked; a size of its own opens Advanced
  useEffect(() => {
    const current = options.plans.find((choice) => choice.current);
    setValues(options.current);
    setPlan(current?.uuid ?? null);
    setAdvanced(!current);
  }, [options]);

  const hasPlans = options.plans.length > 0;
  const showInputs = advanced || !hasPlans;

  const error = (field: Field): string | undefined => {
    const value = values[field];
    const max = options.max[field];

    if (value < 1 && !(value === 0 && options.current[field] === 0)) {
      return t(`${base}.atLeastOne`, {});
    }
    if (max !== null && value > max && value > options.current[field]) {
      return t(`${base}.overMax`, {});
    }

    return undefined;
  };

  const changed = FIELDS.some((field) => values[field] !== options.current[field]);
  const valid = FIELDS.every((field) => !error(field));

  const pickPlan = (uuid: string) => {
    const choice = options.plans.find((option) => option.uuid === uuid);
    if (!choice) return;

    setPlan(uuid);
    setValues({ cpu: choice.cpu, memory: choice.memory, disk: choice.disk });
  };

  // what the owner has left of a resource, under its field
  const left = (field: Field, unit: string) => {
    const max = options.max[field];

    return max === null
      ? t(`${general}.unlimited`, {})
      : t(`${base}.left`, { amount: `${max - values[field]}${unit}` });
  };

  const input = (field: Field, label: string, unit: string) => (
    <NumberInput
      key={field}
      withAsterisk
      label={label}
      description={left(field, unit)}
      inputWrapperOrder={HELPER_UNDER}
      min={0}
      allowDecimal={false}
      value={values[field]}
      error={error(field)}
      rightSection={<span className='theme-input-suffix'>{unit}</span>}
      rightSectionWidth={44}
      rightSectionPointerEvents='none'
      hideControls
      onChange={(value) => setValues((current) => ({ ...current, [field]: Number(value) || 0 }))}
    />
  );

  const doResize = () => {
    setLoading(true);
    resizeServer(server.uuid, values)
      .then(() => {
        addToast(t(`${base}.toast.resized`, {}), 'success');
        updateServer({ limits: { ...server.limits, ...values } });
        onSaved();
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <SettingsSection id='resources' title={t(`${base}.title`, {})} description={t(`${base}.description`, {})}>
      <div className='theme-eco-card'>
        {/* the plans as on Create Server: radio cards, four a row from lg */}
        {hasPlans && !advanced && (
          <div className='theme-choice-grid theme-choice-grid-4'>
            {options.plans.map((choice) => (
              <Choice
                key={choice.uuid}
                radio
                selected={choice.uuid === plan}
                disabled={!choice.fits && !choice.current}
                onSelect={() => pickPlan(choice.uuid)}
              >
                <span className='theme-choice-title'>{choice.name}</span>
                {(choice.current || !choice.fits) && (
                  <span className='theme-choice-text'>
                    {choice.current ? t(`${base}.planCurrent`, {}) : t(`${base}.planUnavailable`, {})}
                  </span>
                )}
                <span className='theme-choice-meta'>
                  {t(`${base}.planMeta`, { cpu: choice.cpu, memory: choice.memory, disk: choice.disk })}
                </span>
              </Choice>
            ))}
          </div>
        )}

        {hasPlans && (
          <Switch
            label={t(`${base}.advanced`, {})}
            description={t(`${base}.advancedHint`, {})}
            checked={advanced}
            onChange={(event) => {
              const next = event.currentTarget.checked;
              setAdvanced(next);
              if (next) setPlan(null);
            }}
          />
        )}

        {showInputs && (
          <div className='grid grid-cols-1 gap-4 lg:grid-cols-3'>
            {input('cpu', t(`${base}.cpu`, {}), '%')}
            {input('memory', t(`${base}.memory`, {}), 'MB')}
            {input('disk', t(`${base}.disk`, {}), 'MB')}
          </div>
        )}

        <div>
          <Button loading={loading} disabled={!changed || !valid} onClick={doResize}>
            {t(`${base}.save`, {})}
          </Button>
        </div>
      </div>
    </SettingsSection>
  );
}

// the server's limits, for whoever may not change them: CPU, memory and disk, and its backups, databases and
// allocations as "used / limit" (the limit alone where the person may not see that list; ∞ for none)
function Limits() {
  const { t } = useTranslations();
  const server = useServerStore((state) => state.server);

  const usage = useResource({
    queryKey: queryKeys.server(server.uuid).settings.usage(),
    queryFn: () => getServerUsage(server.uuid),
    silent: true,
  }).data;

  const size = (megabytes: number) => (megabytes ? bytesToString(mbToBytes(megabytes)) : t(`${general}.unlimited`, {}));
  const count = (used: number | null | undefined, limit: number) => {
    const max = limit ? String(limit) : '∞';
    return used === null || used === undefined ? max : `${used} / ${max}`;
  };

  const limits = [
    {
      label: t(`${general}.limit.cpu`, {}),
      value: server.limits.cpu ? `${server.limits.cpu}%` : t(`${general}.unlimited`, {}),
    },
    { label: t(`${general}.limit.memory`, {}), value: size(server.limits.memory) },
    { label: t(`${general}.limit.disk`, {}), value: size(server.limits.disk) },
    { label: t(`${general}.limit.backups`, {}), value: count(usage?.backups, server.featureLimits.backups) },
    { label: t(`${general}.limit.databases`, {}), value: count(usage?.databases, server.featureLimits.databases) },
    {
      label: t(`${general}.limit.allocations`, {}),
      value: count(usage?.allocations, server.featureLimits.allocations),
    },
  ];

  return (
    <SettingsSection id='resources' title={t(`${general}.limits`, {})}>
      <SettingRows className='theme-eco-list'>
        {limits.map((limit) => (
          <SettingRow key={limit.label} label={limit.label}>
            {limit.value}
          </SettingRow>
        ))}
      </SettingRows>
    </SettingsSection>
  );
}

export default function ResourcesSection() {
  const { resource, usable } = useResizeOptions();

  if (usable && resource.data) {
    return <Resize options={resource.data} onSaved={resource.invalidate} />;
  }

  // the page waits for the first answer; only then is it known that the server cannot be resized here
  return resource.loading && !resource.data ? null : <Limits />;
}
