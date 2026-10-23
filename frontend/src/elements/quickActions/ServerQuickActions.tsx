import { faPlay, faRotateRight, faSkull, faStop } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import ConfirmationModal from '@/elements/modals/ConfirmationModal.tsx';
import { CORE_QUICK_ACTION_CATEGORIES } from '@/lib/quickActions/coreQuickActions.tsx';
import { useQuickActions } from '@/plugins/quick-actions/useQuickActions.ts';
import { usePowerAction } from '@/plugins/server/usePowerAction.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { useServerStore } from '@/stores/server.ts';

/** Mounted by the server router, so these exist exactly as long as a server is being viewed. */
export default function ServerQuickActions() {
  const { t } = useTranslations();
  const { serverState } = useServerStore(useShallow((state) => ({ serverState: state.state })));
  const powerAction = usePowerAction(); // plugged.host: over the API, through the start gates

  const [killConfirmOpen, setKillConfirmOpen] = useState(false);

  const power = CORE_QUICK_ACTION_CATEGORIES.power;

  useQuickActions([
    {
      id: 'server.start',
      category: power,
      label: () => t('common.enum.serverPowerAction.start', {}),
      icon: <FontAwesomeIcon icon={faPlay} />,
      permission: 'control.start',
      isVisible: () => serverState === 'offline',
      perform: () => powerAction('start'),
    },
    {
      id: 'server.stop',
      category: power,
      label: () => t('common.enum.serverPowerAction.stop', {}),
      icon: <FontAwesomeIcon icon={faStop} />,
      permission: 'control.stop',
      isVisible: () => serverState !== 'offline' && serverState !== 'stopping',
      perform: () => powerAction('stop'),
    },
    {
      id: 'server.restart',
      category: power,
      label: () => t('common.enum.serverPowerAction.restart', {}),
      icon: <FontAwesomeIcon icon={faRotateRight} />,
      permission: 'control.restart',
      isVisible: () => serverState === 'running',
      perform: () => powerAction('restart'),
    },
    {
      id: 'server.kill',
      category: power,
      label: () => t('common.enum.serverPowerAction.kill', {}),
      icon: <FontAwesomeIcon icon={faSkull} />,
      danger: true,
      permission: 'control.stop',
      isVisible: () => serverState === 'stopping',
      perform: () => setKillConfirmOpen(true),
    },
  ]);

  return (
    <ConfirmationModal
      opened={killConfirmOpen}
      onClose={() => setKillConfirmOpen(false)}
      title={t('pages.server.console.power.modal.forceStop.title', {})}
      confirm={t('common.button.continue', {})}
      onConfirmed={() => {
        powerAction('kill');
        setKillConfirmOpen(false);
      }}
    >
      {t('pages.server.console.power.modal.forceStop.content', {})}
    </ConfirmationModal>
  );
}
