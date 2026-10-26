// plugged.host: the Pelican panel's "Resources" section (ServerSettings::resourcesSections): a plan,
// or CPU, memory and disk set by hand under "Advanced", within the owner's allowance
// (docs/features/server-settings.md)
import { ReactNode, useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import resizeServer from '@/api/server/settings/resizeServer.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import Choice from '@/elements/input/Choice.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import { serverSettingsResizeSchema } from '@/lib/schemas/server/settings.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { useResizeOptions } from './useSettingsSections.ts';

type Values = z.infer<typeof serverSettingsResizeSchema>;
type Field = keyof Values;

const base = 'pages.server.settings.resize';
const FIELDS: Field[] = ['cpu', 'memory', 'disk'];

export default function ResizeContainer() {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);
  const { resource, usable } = useResizeOptions();
  const options = resource.data;

  const [values, setValues] = useState<Values>({
    cpu: server.limits.cpu,
    memory: server.limits.memory,
    disk: server.limits.disk,
  });
  const [plan, setPlan] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [loading, setLoading] = useState(false);

  // as Pelican: the plan the server has now is picked, a size of its own opens "Advanced"
  useEffect(() => {
    if (!options) return;

    const current = options.plans.find((choice) => choice.current);
    setValues(options.current);
    setPlan(current?.uuid ?? null);
    setAdvanced(!current);
  }, [options]);

  if (!options || !usable) {
    // the section is there while its plans load, as on the other settings pages
    return resource.loading ? (
      <TitleCard title={t(`${base}.title`, {})} description={t(`${base}.description`, {})}>
        <Spinner.Centered />
      </TitleCard>
    ) : null;
  }

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

  // Filament's field hint at the end of the label line: what is left, red once it goes below zero
  const label = (field: Field, text: string, unit: string): ReactNode => {
    const max = options.max[field];
    const left = max === null ? null : max - values[field];

    return (
      <span className='theme-field-label'>
        {/* Filament's required mark, inside the label's first part so the hint keeps its line */}
        <span>
          {text}
          <span className='text-(--mantine-color-error)'> *</span>
        </span>
        <span className='theme-field-hint' data-over={left !== null && left < 0 ? true : undefined}>
          {left === null
            ? t('pages.server.settings.general.unlimited', {})
            : t(`${base}.left`, { amount: `${left}${unit}` })}
        </span>
      </span>
    );
  };

  const input = (field: Field, text: string, unit: string) => (
    <NumberInput
      key={field}
      classNames={{ label: 'theme-field-label-row' }}
      label={label(field, text, unit)}
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
        resource.invalidate();
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setLoading(false));
  };

  return (
    <TitleCard
      title={t(`${base}.title`, {})}
      description={t(`${base}.description`, {})}
      footer={
        <Button loading={loading} disabled={!changed || !valid} onClick={doResize}>
          {t(`${base}.save`, {})}
        </Button>
      }
    >
      {/* Filament's fields 1.5rem apart; the plans are CardSelect's radio cards, four from lg, as on
          Create Server */}
      <div className='flex flex-col gap-6'>
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
          <div className='grid grid-cols-1 gap-6 lg:grid-cols-3'>
            {input('cpu', t(`${base}.cpu`, {}), '%')}
            {input('memory', t(`${base}.memory`, {}), 'MB')}
            {input('disk', t(`${base}.disk`, {}), 'MB')}
          </div>
        )}
      </div>
    </TitleCard>
  );
}
