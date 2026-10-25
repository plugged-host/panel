// plugged.host: Filament's wizard (".fi-sc-wizard", the Pelican panel's server setup) on Theme v2's
// section card: numbered steps across the top, one step's fields, Back / Next, and the submit
// button on the last step. Enter goes to the next step. (docs/features/minecraft.md)
import { faCheck } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { FormEvent, ReactNode } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Card from '@/elements/data-display/Card.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export interface WizardStep {
  key: string;
  title: string;
  description?: string;
  content: ReactNode;
  /** Whether the step may be left forward; it shows its own errors when not. */
  validate?: () => boolean;
}

interface Props {
  steps: WizardStep[];
  active: number;
  onActiveChange: (index: number) => void;
  submitLabel: string;
  submitting?: boolean;
  onSubmit: () => void;
}

export default function Wizard({ steps, active, onActiveChange, submitLabel, submitting, onSubmit }: Props) {
  const { t } = useTranslations();
  const current = Math.min(active, steps.length - 1);
  const last = current === steps.length - 1;

  const next = () => {
    if (steps[current]?.validate && !steps[current].validate()) return;
    onActiveChange(current + 1);
  };

  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (submitting) return;
    if (!last) {
      next();
    } else if (!steps[current]?.validate || steps[current].validate()) {
      onSubmit();
    }
  };

  return (
    <form onSubmit={submit}>
      <Card p={0}>
        <ol className='theme-wizard-steps'>
          {steps.map((step, index) => (
            <li key={step.key} className='theme-wizard-cell' data-active={index === current || undefined}>
              <button
                type='button'
                className='theme-wizard-step'
                data-active={index === current || undefined}
                data-completed={index < current || undefined}
                aria-current={index === current ? 'step' : undefined}
                // like Filament, only the steps already done can be gone back to
                disabled={index >= current}
                onClick={() => onActiveChange(index)}
              >
                <span className='theme-wizard-step-number'>
                  {index < current ? <FontAwesomeIcon icon={faCheck} /> : index + 1}
                </span>
                <span className='theme-wizard-step-text'>
                  <span className='theme-wizard-step-label'>{step.title}</span>
                  {step.description && <span className='theme-wizard-step-description'>{step.description}</span>}
                </span>
              </button>
            </li>
          ))}
        </ol>

        <div className='theme-wizard-body'>{steps[current]?.content}</div>

        <div className='theme-wizard-footer'>
          {current > 0 ? (
            <Button variant='default' onClick={() => onActiveChange(current - 1)} disabled={submitting}>
              {t('common.button.back', {})}
            </Button>
          ) : (
            <span />
          )}
          {last ? (
            <Button type='submit' loading={submitting}>
              {submitLabel}
            </Button>
          ) : (
            <Button type='submit'>{t('common.button.next', {})}</Button>
          )}
        </div>
      </Card>
    </form>
  );
}
