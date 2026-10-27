// plugged.host: a startup command of the user's own, checked before it is applied (docs/features/startup-commands.md)
import { ReactNode, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import applyCustomCommand, { type CustomCommandResult } from '@/api/server/startup/applyCustomCommand.ts';
import Button from '@/elements/buttons/Button.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import { toCamelCase } from '@/lib/serialization/transformers.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

const base = 'pages.server.settings.startup';

// the refusals the panel names (shared/src/startup_command.rs) by their string keys; any other reads as `other`
const REASONS = new Set([
  'empty',
  'tooLong',
  'lineBreak',
  'controlCharacter',
  'unclosedPlaceholder',
  'unclosedQuote',
  'unclosedSubstitution',
  'trailingBackslash',
  'danglingOperator',
  'tooDeep',
  'unknownVariable',
  'commandSubstitution',
  'processSubstitution',
  'eval',
  'heredoc',
  'pipeToShell',
  'parameterExpansion',
  'escapeSequence',
  'background',
  'abuse',
  'neoUnavailable',
  'neoBusy',
  'neoLimit',
  'neoUnlinked',
  'neoModel',
  'neoUnreadable',
  'neoUnsafe',
  'neoNotStarting',
  'neoUnsure',
]);

const HELPER_UNDER: ('label' | 'input' | 'description' | 'error')[] = ['label', 'input', 'description', 'error'];

interface Props {
  command: string;
  onChange: (command: string) => void;
  /** The label line's action ("Enable Preview"), when no list of commands carries it. */
  action?: ReactNode;
}

export default function CustomStartupCommand({ command, onChange, action }: Props) {
  const { t } = useTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const updateServer = useServerStore((state) => state.updateServer);

  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<CustomCommandResult | null>(null);

  const apply = () => {
    setChecking(true);
    setResult(null);

    applyCustomCommand(server.uuid, command)
      .then((answer) => {
        if (answer.applied) {
          addToast(t(`${base}.check.applied`, {}), 'success');
          updateServer({ startup: command });
        } else {
          setResult(answer);
        }
      })
      .catch((msg) => {
        addToast(httpErrorToHuman(msg), 'error');
      })
      .finally(() => setChecking(false));
  };

  const reason = (code: string, detail: string | null) => {
    const key = toCamelCase(code);

    return t(`${base}.check.reasons.${REASONS.has(key) ? key : 'other'}`, { detail: detail ?? '' });
  };

  const refusal = result?.refusal;
  const problems = result?.verdict?.problems ?? [];
  const summary = result?.verdict?.summary;

  return (
    <div className='mt-6'>
      <div className='theme-field-action-wrap'>
        <TextArea
          label={t(`${base}.custom`, {})}
          description={t(`${base}.check.description`, {})}
          inputWrapperOrder={HELPER_UNDER}
          value={command}
          autosize
          dir='ltr'
          onChange={(e) => {
            onChange(e.currentTarget.value);
            setResult(null);
          }}
        />
        {action}
      </div>

      <div className='mt-4'>
        <Button loading={checking} disabled={!command.trim() || command === server.startup} onClick={apply}>
          {t(`${base}.check.button`, {})}
        </Button>
      </div>

      {refusal && (
        <Alert color='red' title={t(`${base}.check.refused`, {})} className='mt-4'>
          <ul className='list-disc ps-5'>
            <li>{reason(refusal.code, refusal.detail)}</li>
            {problems.map((problem, index) => (
              <li key={`${index}-${problem}`}>{problem}</li>
            ))}
          </ul>
          {summary && <p className='mt-2'>{summary}</p>}
        </Alert>
      )}
    </div>
  );
}
