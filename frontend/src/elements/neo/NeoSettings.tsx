// plugged.host: neo's settings inside the chat (the Pelican panel's settings view): the two
// switches, where the limits stand, deleting the history, and what is sent where
import { useQueryClient } from '@tanstack/react-query';
import { Fragment, ReactNode, useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Switch from '@/elements/input/Switch.tsx';
import { barClass, orderedWindows, percentOf } from '@/elements/neo/NeoComposer.tsx';
import NeoHead from '@/elements/neo/NeoHead.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import type { NeoInfo, NeoUsage, NeoWindow } from '@/lib/schemas/user/neo.ts';
import { useNeoText, useNeoUsage } from '@/plugins/useNeo.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useGlobalStore } from '@/stores/global.ts';
import { deleteAllNeoChats, NeoChat, updateNeo } from '@/stores/neo.ts';

type Info = Omit<NeoInfo, 'conversation'>;

// how long the delete button waits for the second tap
const CONFIRM_MS = 4000;

function Bar({ label, window }: { label: string; window: NeoWindow }) {
  const text = useNeoText();
  const percent = percentOf(window);

  return (
    <div className='theme-neo-usage'>
      <div className='theme-neo-usage-head'>
        <span>{label}</span>
        <span>
          {percent}% · {text.resets(window)}
        </span>
      </div>
      <div
        className={`theme-neo-bar theme-neo-bar-lg ${barClass(percent)}`}
        role='progressbar'
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <i style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

function Usage({ usage, loading }: { usage: NeoUsage | null | undefined; loading: boolean }) {
  const { t } = useTranslations();
  const text = useNeoText();

  if (loading) return null;

  if (usage && !usage.linked) {
    return <div className='theme-neo-set-hint'>{t('elements.neo.settings.unlinked', { name: text.name })}</div>;
  }

  if (!usage || (usage.mode !== 'models' && !usage.limits)) {
    return <div className='theme-neo-set-hint'>{t('elements.neo.settings.unavailable', {})}</div>;
  }

  if (usage.mode === 'models') {
    // limits by model: only the limited ones have a window
    return (
      <>
        {usage.models
          .filter((model) => model.allowed && model.limited && model.window)
          .map((model) => (
            <Bar
              key={model.key}
              label={`${model.label} · ${text.windowLabel(model.window as NeoWindow)}`}
              window={model.window as NeoWindow}
            />
          ))}
        <div className='theme-neo-set-hint'>{t('elements.neo.settings.unlimitedModels', {})}</div>
      </>
    );
  }

  return (
    <>
      {orderedWindows(usage.limits ?? {}).map(([kind, window]) => (
        <Bar key={kind} label={text.windowLabel(window)} window={window} />
      ))}
    </>
  );
}

export default function NeoSettings({ info, chat }: { info: Info; chat: NeoChat }) {
  const { t, tReact } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const text = useNeoText();
  const onNoprofit = useGlobalStore((state) => state.settings.noprofit.enabled);
  const usage = useNeoUsage(true);
  const [saving, setSaving] = useState<string | null>(null);
  const [sure, setSure] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    if (!sure) return;

    const timer = setTimeout(() => setSure(false), CONFIRM_MS);
    return () => clearTimeout(timer);
  }, [sure]);

  const toggle = (key: 'enabled' | 'autoApprove', value: boolean) => {
    setSaving(key);
    updateNeo({ [key]: value })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(null));
  };

  const deleteHistory = () => {
    if (!sure) {
      setSure(true);
      return;
    }

    setSure(false);
    setDeleting(true);
    deleteAllNeoChats()
      .then(() => {
        addToast(t('elements.neo.settings.deleted', { name: text.name }), 'success');
        return queryClient.invalidateQueries({ queryKey: queryKeys.user.neo.allConversations() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setDeleting(false));
  };

  const switches = [
    {
      key: 'enabled' as const,
      checked: info.preferences.enabled,
      title: t('elements.neo.settings.enabled', { name: text.name }),
      hint: t('elements.neo.settings.enabledHint', { name: text.name }),
    },
    {
      key: 'autoApprove' as const,
      checked: info.preferences.autoApprove,
      title: t('elements.neo.settings.autoApprove', { name: text.name }),
      hint: t('elements.neo.settings.autoApproveHint', { name: text.name }),
    },
  ];

  const mail = (email: string) => <a href={`mailto:${email}`}>{email}</a>;
  // where a person answers: support, legal questions, privacy requests (each only when set)
  const contacts = [
    info.supportUrl && {
      key: 'support',
      node: (
        <a href={info.supportUrl} target='_blank' rel='noreferrer'>
          {t('elements.neo.settings.support', {})}
        </a>
      ),
    },
    info.legalEmail && {
      key: 'legal',
      node: tReact('elements.neo.consent.legal', { email: mail(info.legalEmail) }),
    },
    info.privacyEmail && {
      key: 'privacy',
      node: tReact('elements.neo.consent.privacy', { email: mail(info.privacyEmail) }),
    },
  ].filter((contact): contact is { key: string; node: ReactNode } => !!contact);

  return (
    <>
      <NeoHead back sub={t('elements.neo.header.settings', { name: text.name })} />
      <div className='theme-neo-settings'>
        <div className='theme-neo-set-card'>
          {switches.map((entry) => (
            <div key={entry.key} className='theme-neo-set-row'>
              <div className='theme-neo-set-text'>
                <div className='theme-neo-set-title'>{entry.title}</div>
                <div className='theme-neo-set-hint'>{entry.hint}</div>
              </div>
              <Switch
                checked={entry.checked}
                disabled={saving !== null}
                aria-label={entry.title}
                onChange={(event) => toggle(entry.key, event.currentTarget.checked)}
              />
            </div>
          ))}
        </div>

        <div className='theme-neo-set-card'>
          <div className='theme-neo-set-row'>
            <div className='theme-neo-set-text'>
              <div className='theme-neo-set-title'>{t('elements.neo.settings.usage', {})}</div>
              <div className='theme-neo-set-hint'>
                {onNoprofit
                  ? t('elements.neo.settings.usageHintNoprofit', {})
                  : t('elements.neo.settings.usageHint', { name: text.name })}
              </div>
            </div>
          </div>
          <Usage usage={usage.data} loading={usage.isLoading} />
        </div>

        <div className='theme-neo-set-card'>
          <div className='theme-neo-set-row'>
            <div className='theme-neo-set-text'>
              <div className='theme-neo-set-title'>{t('elements.neo.settings.history', {})}</div>
              <div className='theme-neo-set-hint'>{t('elements.neo.settings.historyHint', {})}</div>
            </div>
          </div>
          <div>
            <Button size='sm' variant='default' loading={deleting} disabled={chat.working} onClick={deleteHistory}>
              {sure
                ? t('elements.neo.button.confirmDelete', {})
                : t('elements.neo.button.deleteHistory', { name: text.name })}
            </Button>
          </div>
        </div>

        <p className='theme-neo-small'>
          {info.providerDisclosure
            ? t('elements.neo.settings.privacyNamed', { name: text.name, providers: info.providerDisclosure })
            : t('elements.neo.settings.privacyGeneric', { name: text.name })}
        </p>
        {contacts.length > 0 && (
          <p className='theme-neo-small'>
            {contacts.map((contact, index) => (
              <Fragment key={contact.key}>
                {index > 0 && <br />}
                {contact.node}
              </Fragment>
            ))}
          </p>
        )}
      </div>
    </>
  );
}
