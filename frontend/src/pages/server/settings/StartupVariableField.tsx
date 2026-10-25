// plugged.host: one startup variable as a field of the Pelican panel's "Server Variables" (its
// StartupVariable field): the variable's name with its {{ENV}} on the label line, its description
// under the field, saved on its own when it loses focus or, for switches and lists, when it
// changes (docs/features/server-settings.md)
import { faReply } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ReactNode, useEffect, useState } from 'react';
import { z } from 'zod';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import PasswordInput from '@/elements/input/PasswordInput.tsx';
import Select from '@/elements/input/Select.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { ignorePasswordManagerProps } from '@/lib/passwordManager.ts';
import { serverVariableSchema } from '@/lib/schemas/server/startup.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Variable = z.infer<typeof serverVariableSchema>;

// Filament's helper text sits under the field
const HELPER_UNDER: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

const TRUE_FALSE = ['in:true,false', 'in:false,true'];
const ONE_ZERO = ['in:1,0', 'in:0,1'];

export default function StartupVariableField({
  variable,
  disabled,
  onSave,
}: {
  variable: Variable;
  disabled: boolean;
  // resolves false when the value was not taken, so the field shows the stored one again
  onSave: (value: string) => Promise<boolean>;
}) {
  const { t, language } = useTranslations();
  const [value, setValue] = useState(variable.value);
  const [saving, setSaving] = useState(false);

  useEffect(() => setValue(variable.value), [variable.value]);

  const name = variable.nameTranslations?.[language] || variable.name;
  const description = variable.descriptionTranslations?.[language] || variable.description;
  const locked = disabled || saving || !variable.isEditable;
  const required = variable.rules.includes('required');

  const save = (next: string) => {
    setValue(next);
    if (next === variable.value) return;

    setSaving(true);
    onSave(next)
      .then((taken) => {
        if (!taken) setValue(variable.value);
      })
      .finally(() => setSaving(false));
  };

  const isBoolean =
    variable.rules.includes('boolean') ||
    (variable.rules.includes('string') && [...TRUE_FALSE, ...ONE_ZERO].some((rule) => variable.rules.includes(rule)));

  if (isBoolean) {
    const words = TRUE_FALSE.some((rule) => variable.rules.includes(rule));

    return (
      <Switch
        label={name}
        description={description?.md()}
        checked={value === '1' || value === 'true'}
        disabled={locked}
        onChange={(e) => save(e.currentTarget.checked ? (words ? 'true' : '1') : words ? 'false' : '0')}
      />
    );
  }

  const label: ReactNode = (
    <span className='theme-field-label'>
      <span>
        {name}
        {required && <span className='text-(--mantine-color-error)'> *</span>}
      </span>
      <span className='theme-field-hint font-mono'>{`{{${variable.envVariable}}}`}</span>
    </span>
  );
  const common = {
    id: variable.envVariable,
    label,
    classNames: { label: 'theme-field-label-row' },
    description: description?.md(),
    inputWrapperOrder: HELPER_UNDER,
    disabled: locked,
  };

  const options = variable.rules.find((rule) => rule.startsWith('in:'));
  if (options) {
    return (
      <Select
        {...common}
        data={options
          .replace('in:', '')
          .split(',')
          .map((option) => ({ value: option, label: option }))}
        value={value}
        onChange={(next) => save(next ?? '')}
      />
    );
  }

  if (['integer', 'int', 'numeric', 'num'].some((rule) => variable.rules.includes(rule))) {
    return (
      <NumberInput
        {...common}
        placeholder={variable.defaultValue ?? ''}
        value={value}
        onChange={(next) => setValue(String(next))}
        onBlur={() => save(value)}
      />
    );
  }

  if (variable.isSecret) {
    return (
      <PasswordInput
        {...common}
        placeholder={variable.defaultValue ?? ''}
        value={value}
        onChange={(e) => setValue(e.currentTarget.value)}
        onBlur={() => save(value)}
      />
    );
  }

  return (
    <TextInput
      {...common}
      {...ignorePasswordManagerProps}
      placeholder={variable.defaultValue ?? ''}
      value={value}
      onChange={(e) => setValue(e.currentTarget.value)}
      onBlur={() => save(value)}
      rightSection={
        <Tooltip label={t('common.tooltip.resetToDefault', {})}>
          <ActionIcon
            variant='subtle'
            color='gray'
            disabled={!variable.defaultValue || locked || value === variable.defaultValue}
            onClick={() => save(variable.defaultValue ?? '')}
          >
            <FontAwesomeIcon icon={faReply} />
          </ActionIcon>
        </Tooltip>
      }
    />
  );
}
