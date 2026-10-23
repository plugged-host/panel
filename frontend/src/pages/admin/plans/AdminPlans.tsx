// plugged.host: Admin › Plans, the resource presets players create servers with
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
import PlanRow from './PlanRow.tsx';

function PlansContainer() {
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
    queryKey: queryKeys.admin.plans.all(),
    fetcher: getPlans,
  });

  return (
    <AdminContentContainer
      title={t('pages.admin.plans.title', {})}
      search={search}
      setSearch={setSearch}
      contentRight={
        <AdminCan action='plans.create'>
          <Button
            onClick={() => navigate('/admin/plans/new')}
            color='blue'
            leftSection={<FontAwesomeIcon icon={faPlus} />}
          >
            {t('common.button.create', {})}
          </Button>
        </AdminCan>
      }
    >
      <Table
        columns={planTableColumns()}
        loading={loading}
        error={error}
        pagination={plans}
        onPageSelect={setPage}
        onPerPageSelect={setPerPage}
      >
        {plans?.data.map((plan) => (
          <PlanRow key={plan.uuid} plan={plan} />
        ))}
      </Table>
    </AdminContentContainer>
  );
}

function PlanView() {
  const params = useParams<'id'>();

  const resource = useResource({
    queryKey: queryKeys.admin.plans.detail(params.id!),
    queryFn: () => getPlan(params.id!),
  });

  return <ResourceView resource={resource}>{(plan) => <PlanCreateOrUpdate contextPlan={plan} />}</ResourceView>;
}

export default function AdminPlans() {
  return (
    <Routes>
      <Route path='/' element={<PlansContainer />} />
      <Route path='/:id' element={<PlanView />} />
      <Route element={<AdminPermissionGuard permission='plans.create' />}>
        <Route path='/new' element={<PlanCreateOrUpdate />} />
      </Route>
    </Routes>
  );
}
