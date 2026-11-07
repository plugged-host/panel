// plugged.host: one support ticket as a full page (docs/features/tickets.md).
import { useNavigate, useParams } from 'react-router';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { ThreadModal } from './DashboardTickets.tsx';

export default function DashboardTicket() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const { id } = useParams<'id'>();

  return (
    <AccountContentContainer title={t('pages.account.tickets.thread.title', {})}>
      <ThreadModal uuid={id ?? null} inline onClose={() => navigate('/account/support')} />
    </AccountContentContainer>
  );
}
