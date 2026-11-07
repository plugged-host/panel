// plugged.host: one support ticket as a full page in Admin › Tickets (docs/features/tickets.md).
import { useNavigate, useParams } from 'react-router';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { ThreadModal } from './AdminTickets.tsx';

export default function AdminTicket() {
  const { t } = useTranslations();
  const navigate = useNavigate();
  const { id } = useParams<'id'>();

  return (
    <AdminContentContainer title={t('pages.admin.tickets.thread.title', {})}>
      <ThreadModal ticket={id ? { uuid: id } : null} inline onClose={() => navigate('/admin/tickets')} />
    </AdminContentContainer>
  );
}
