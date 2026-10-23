import { faDollarSign, faFont } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ReactNode, useMemo, useState } from 'react';
import { z } from 'zod';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Autocomplete from '@/elements/input/Autocomplete.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { serverScheduleStepDynamicSchema, serverScheduleStepVariableSchema } from '@/lib/schemas/server/schedules.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { stepWrites, triggerWrites } from '../variableAnalysis.ts';

interface ScheduleDynamicParameterInputProps<
  N extends boolean,
  S extends boolean,
  Param = S extends true
    ? z.infer<typeof serverScheduleStepDynamicSchema>
    : z.infer<typeof serverScheduleStepVariableSchema>,
> {
  textArea?: boolean;
  withAsterisk?: boolean;
  label?: string;
  placeholder?: string;
  className?: string;
  error?: ReactNode;
  output?: boolean;
  allowNull?: N;
  allowString?: S;
  value: N extends true ? Param | null : Param;
  onChange: (value: N extends true ? Param | null : Param) => void;
}

export default function ScheduleDynamicParameterInput<N extends boolean = false, S extends boolean = true>({
  textArea = false,
  className,
  output = false,
  allowNull = false as never,
  allowString = true as never,
  value,
  onChange,
  ...rest
}: ScheduleDynamicParameterInputProps<N, S>) {
  const { t } = useTranslations();
  const schedule = useServerStore((server) => server.schedule);
  const scheduleSteps = useServerStore((server) => server.scheduleSteps);

  const [nullModePreference, setNullModePreference] = useState<'text' | 'variable'>(
    value && typeof value === 'object' ? 'variable' : allowString ? 'text' : 'variable',
  );

  const outputVariables = useMemo(() => {
    if (!schedule) {
      return [];
    }

    return [
      ...new Set([
        ...triggerWrites(schedule.triggers).map((write) => write.variable),
        ...scheduleSteps.flatMap((step) => stepWrites(step.action).map((write) => write.variable)),
      ]),
    ];
  }, [schedule, scheduleSteps]);

  const mode: 'text' | 'variable' = !allowString
    ? 'variable'
    : value && typeof value === 'object'
      ? 'variable'
      : typeof value === 'string'
        ? 'text'
        : nullModePreference;

  const displayedValue: string =
    mode === 'variable'
      ? value && typeof value === 'object'
        ? value.variable
        : ''
      : typeof value === 'string'
        ? value
        : '';

  const handleChange = (input: string) => {
    if (allowNull && input === '') {
      onChange(null as never);
    } else if (mode === 'variable') {
      onChange({ variable: input } as never);
    } else {
      onChange(input as never);
    }
  };

  const toggleMode = () => {
    const newMode = mode === 'variable' ? 'text' : 'variable';
    setNullModePreference(newMode);

    if (allowNull && displayedValue === '') {
      onChange(null as never);
    } else if (newMode === 'variable') {
      onChange({ variable: displayedValue } as never);
    } else {
      onChange(displayedValue as never);
    }
  };

  const modeToggle = allowString ? (
    <Tooltip
      label={
        mode === 'variable'
          ? t('elements.scheduleDynamicInput.usePlainText', {})
          : t('elements.scheduleDynamicInput.useVariable', {})
      }
    >
      <ActionIcon variant='subtle' color='gray' onClick={toggleMode}>
        <FontAwesomeIcon icon={mode === 'variable' ? faFont : faDollarSign} />
      </ActionIcon>
    </Tooltip>
  ) : null;

  const placeholder = allowNull ? t('elements.scheduleDynamicInput.optionalPlaceholder', {}) : rest.placeholder;

  if (mode === 'variable') {
    return (
      <Autocomplete
        className={className}
        description={
          output
            ? t('elements.scheduleDynamicInput.writeVariable', {})
            : t('elements.scheduleDynamicInput.enterVariable', {})
        }
        value={displayedValue}
        onChange={handleChange}
        data={outputVariables}
        rightSection={modeToggle}
        rightSectionPointerEvents='all'
        {...rest}
        placeholder={placeholder}
      />
    );
  }

  if (textArea) {
    return (
      <TextArea
        className={className}
        value={displayedValue}
        onChange={(e) => handleChange(e.target.value)}
        rightSection={modeToggle}
        rightSectionPointerEvents='all'
        {...rest}
        placeholder={placeholder}
      />
    );
  }

  return (
    <TextInput
      className={className}
      value={displayedValue}
      onChange={(e) => handleChange(e.target.value)}
      rightSection={modeToggle}
      rightSectionPointerEvents='all'
      {...rest}
      placeholder={placeholder}
    />
  );
}
