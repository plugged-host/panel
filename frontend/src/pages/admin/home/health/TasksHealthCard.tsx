// plugged.host: Admin › Overview › Health: when each background task last ran and whether it
// failed (docs/features/health.md).
import { faClock } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import getTasksHealth from '@/api/admin/system/health/getTasksHealth.ts';
import Badge from '@/elements/data-display/Badge.tsx';
import Table, { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import ResourceView from '@/elements/ResourceView.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { useResource } from '@/plugins/resource/useResource.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function TasksHealthCard() {
  const { t } = useTranslations();

  const tasks = useResource({
    queryKey: queryKeys.admin.health.tasks(),
    queryFn: getTasksHealth,
  });

  return (
    <TitleCard title={t('pages.admin.home.tabs.health.page.card.tasks', {})} icon={<FontAwesomeIcon icon={faClock} />}>
      <ResourceView resource={tasks}>
        {(health) =>
          !health.primary ? (
            <Text size='sm' c='dimmed'>
              {t('pages.admin.home.tabs.health.page.tasksNotPrimary', {})}
            </Text>
          ) : (
            <Table
              columns={[
                t('common.table.columns.name', {}),
                t('pages.admin.home.tabs.health.page.table.lastRun', {}),
                t('common.table.columns.status', {}),
              ]}
            >
              {health.tasks.map((task) => (
                <TableRow key={task.name}>
                  <TableData>
                    <Code>{task.name}</Code>
                  </TableData>
                  <TableData>
                    <FormattedTimestamp timestamp={task.lastRun} />
                  </TableData>
                  <TableData>
                    {task.error ? (
                      <Text size='sm' c='red' lineClamp={3} title={task.error}>
                        {task.error}
                      </Text>
                    ) : (
                      <Badge color='green' variant='light'>
                        {t('pages.admin.home.tabs.health.page.taskOk', {})}
                      </Badge>
                    )}
                  </TableData>
                </TableRow>
              ))}
            </Table>
          )
        }
      </ResourceView>
    </TitleCard>
  );
}
