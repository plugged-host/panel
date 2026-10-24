// plugged.host: the pill in the window's corner that opens neo's chat (the Pelican panel's
// ".neo-launcher"): it says when neo works, waits for an approval or has finished, and shrinks to
// a faded icon while the person has neo turned off
import { faSpinner, faWandMagicSparkles } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import classNames from 'classnames';
import { forwardRef } from 'react';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { useNeoName } from '@/plugins/useNeo.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { NeoPlace, openNeo, useNeoChat } from '@/stores/neo.ts';

const NeoLauncher = forwardRef<HTMLButtonElement, { place: NeoPlace }>(({ place }, ref) => {
  const { t } = useTranslations();
  const name = useNeoName();
  const working = useNeoChat(place.key, (chat) => chat.working);
  const attention = useNeoChat(place.key, (chat) => chat.attention);
  const off = useNeoChat(place.key, (chat) => chat.info?.blocked === 'turned_off');

  if (off) {
    const label = t('elements.neo.launcher.off', { name });

    return (
      <Tooltip label={label} position='left'>
        <button
          ref={ref}
          type='button'
          className='theme-neo-launcher theme-neo-launcher-off'
          aria-label={label}
          onClick={() => openNeo()}
        >
          <FontAwesomeIcon icon={faWandMagicSparkles} />
        </button>
      </Tooltip>
    );
  }

  const label = working
    ? t('elements.neo.launcher.working', {})
    : attention === 'approve'
      ? t('elements.neo.launcher.approve', {})
      : attention === 'done'
        ? t('elements.neo.launcher.done', {})
        : t('elements.neo.launcher.open', { name });

  return (
    // the label is hidden on phones, so the button carries it as its name too
    <button ref={ref} type='button' className='theme-neo-launcher' aria-label={label} onClick={() => openNeo()}>
      <FontAwesomeIcon icon={working ? faSpinner : faWandMagicSparkles} spin={working} />
      <span className='theme-neo-launcher-label'>{label}</span>
      {attention && !working && (
        <span
          className={classNames(
            'theme-neo-launcher-dot',
            attention === 'approve' ? 'theme-neo-dot-approve' : 'theme-neo-dot-done',
          )}
        />
      )}
    </button>
  );
});

export default NeoLauncher;
