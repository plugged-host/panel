// plugged.host: what neo's chat shows instead of the conversation (the Pelican panel's views):
// the notice to agree to first, neo turned off by the person, neo not offered here, loading
import { useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import type { NeoInfo } from '@/lib/schemas/user/neo.ts';
import { useNeoName } from '@/plugins/useNeo.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { loadNeo, NeoPlace, updateNeo } from '@/stores/neo.ts';

type Info = Omit<NeoInfo, 'conversation'>;

/** Saves a preference from a button, with the button's loading state and a toast when it fails. */
function useSave() {
  const { addToast } = useToast();
  const [saving, setSaving] = useState(false);

  const save = (changes: Parameters<typeof updateNeo>[0]) => {
    setSaving(true);
    updateNeo(changes)
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setSaving(false));
  };

  return { saving, save };
}

/** The notice before the first chat: what neo does, what is sent where; agreeing starts the chat. */
export function NeoConsent({ info }: { info: Info }) {
  const { t, tReact } = useTranslations();
  const name = useNeoName();
  const { saving, save } = useSave();

  const mail = (email: string) => <a href={`mailto:${email}`}>{email}</a>;

  return (
    <div className='theme-neo-consent'>
      <h3>{t('elements.neo.consent.title', {})}</h3>
      <ul>
        <li>
          {info.providerDisclosure
            ? t('elements.neo.consent.bodyNamed', { name, providers: info.providerDisclosure })
            : t('elements.neo.consent.bodyGeneric', { name })}
        </li>
        <li>{t('elements.neo.consent.approvals', { name })}</li>
      </ul>
      <Button loading={saving} onClick={() => save({ acknowledged: true })}>
        {t('elements.neo.button.agree', {})}
      </Button>
      {(info.legalEmail || info.privacyEmail) && (
        <p className='theme-neo-small'>
          {info.legalEmail && tReact('elements.neo.consent.legal', { email: mail(info.legalEmail) })}
          {info.legalEmail && info.privacyEmail && <br />}
          {info.privacyEmail && tReact('elements.neo.consent.privacy', { email: mail(info.privacyEmail) })}
        </p>
      )}
    </div>
  );
}

/** The person turned neo off: one button turns it back on. */
export function NeoOff() {
  const { t } = useTranslations();
  const name = useNeoName();
  const { saving, save } = useSave();

  return (
    <div className='theme-neo-body'>
      <div className='theme-neo-thread'>
        <div className='theme-neo-empty'>
          <div>
            <div className='theme-neo-greet'>{t('elements.neo.launcher.off', { name })}</div>
            <div>{t('elements.neo.blocked.offBody', {})}</div>
          </div>
          <Button loading={saving} onClick={() => save({ enabled: true })}>
            {t('elements.neo.button.turnOn', { name })}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** A note in the middle of the chat: neo not offered here, or the chat could not be loaded. */
export function NeoNote({ title, body, retry }: { title: string; body?: string | null; retry?: NeoPlace }) {
  const { t } = useTranslations();

  return (
    <div className='theme-neo-body'>
      <div className='theme-neo-thread'>
        <div className='theme-neo-empty'>
          <div>
            <div className='theme-neo-greet'>{title}</div>
            {body && <div>{body}</div>}
          </div>
          {retry && (
            <Button variant='default' onClick={() => void loadNeo(retry)}>
              {t('elements.neo.button.retry', {})}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

export function NeoLoading() {
  return (
    <div className='theme-neo-body'>
      <div className='theme-neo-thread'>
        <div className='theme-neo-empty'>
          <Spinner />
        </div>
      </div>
    </div>
  );
}
