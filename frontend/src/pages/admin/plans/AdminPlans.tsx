// plugged.host: Admin › Plans, the resource presets players create servers with, and Admin › Dedicated
// Plans, the paid ones (docs/features/dedicated.md): one page each, never mixed
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Route, Routes, useNavigate, useParams } from 'react-router';
import getPlan from '@/api/admin/plans/getPlan.ts';
import getPlans from '@/api/admin/plans/getPlans.ts';
import Button from '@/elements/buttons/Button.tsx';
import { AdminCan } from '@/elements/Can.tsx';
import AdminContentContainer from '@/elements/containers/AdminContentContainer.tsx';
import Table from '@/elements/data-display/Table.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { planTableColumns } from '@/lib/tableColumns.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useSearchablePaginatedTable } from '@/plugins/resource/useSearchablePaginatedTable.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AdminPermissionGuard from '@/routers/guards/AdminPermissionGuard.tsx';
import PlanCreateOrUpdate from './PlanCreateOrUpdate.tsx';
import PlanRow, { planBasePath } from './PlanRow.tsx';

function PlansContainer({ dedicated }: { dedicated: boolean }) {
  const { t } = useTranslations();
  const navigate = useNavigate();

  const {
    data: plans,
    loading,
    error,
    search,
    setSearch,
    setPage,
    setPerPage,
  } = useSearchablePaginatedTable({
    queryKey: [...queryKeys.admin.plans.all(), { dedicated }],
    fetcher: (page, search, perPage) => getPlans(page, search, perPage, dedicated),
  });

  return (
    <AdminContentContainer
      title={dedicated ? t('pages.admin.plans.dedicatedTitle', {}) : t('pages.admin.plans.title', {})}
      subtitle={dedicated ? t('pages.admin.plans.dedicatedSubtitle', {}) : undefined}
      search={search}
      setSearch={setSearch}
      contentRight={
        <AdminCan action='plans.create'>
          <Button
            onClick={() => navigate(`${planBasePath(dedicated)}/new`)}
            color='blue'
            leftSection={<FontAwesomeIcon icon={faPlus} />}
          >
            {t('common.button.create', {})}
          </Button>
        </AdminCan>
      }
    >
      <Table
        columns={planTableColumns(dedicated)}
        loading={loading}
        error={error}
        pagination={plans}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {plans?.data.map((plan) => (
          <PlanRow key={plan.uuid} plan={plan} dedicated={dedicated} />
        ))}
      </Table>
    </AdminContentContainer>
  );
}

function PlanView({ dedicated }: { dedicated: boolean }) {
  const params = useParams<'id'>();

  const resource = useResource({
    queryKey: queryKeys.admin.plans.detail(params.id!),
    queryFn: () => getPlan(params.id!),
  });

  return (
    <ResourceView resource={resource}>
      {(plan) => <PlanCreateOrUpdate contextPlan={plan} dedicated={dedicated} />}
    </ResourceView>
  );
}

export default function AdminPlans({ dedicated = false }: { dedicated?: boolean }) {
  return (
    <Routes>
      <Route path='/' element={<PlansContainer dedicated={dedicated} />} />
      <Route path='/:id' element={<PlanView dedicated={dedicated} />} />
      <Route element={<AdminPermissionGuard permission='plans.create' />}>
        <Route path='/new' element={<PlanCreateOrUpdate dedicated={dedicated} />} />
      </Route>
    </Routes>
  );
}

// plugged.host: Admin › Dedicated Plans
export function AdminDedicatedPlans() {
  return <AdminPlans dedicated />;
}
