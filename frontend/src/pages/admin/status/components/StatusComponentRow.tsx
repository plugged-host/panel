// plugged.host: Admin › Status › Components: one component.
import { faPen } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import Badge from '@/elements/data-display/Badge.tsx';
import { TableData, TableRow } from '@/elements/data-display/Table.tsx';
import TableLink from '@/elements/data-display/TableLink.tsx';
import ContextMenu, { ContextMenuToggle } from '@/elements/overlays/ContextMenu.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import BooleanText from '@/elements/typography/BooleanText.tsx';
import Text from '@/elements/typography/Text.tsx';
import { AdminStatusComponent } from '@/lib/schemas/admin/status.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { componentStatusColorMapping, componentStatusLabelMapping } from '../statusLabels.ts';
import ComponentEditModal from './modals/ComponentEditModal.tsx';

const KIND_KEYS = {
  site: 'pages.admin.status.enum.componentKind.site',
  panel: 'pages.admin.status.enum.componentKind.panel',
  node: 'pages.admin.status.enum.componentKind.node',
} as const;

export default function StatusComponentRow({ component }: { component: AdminStatusComponent }) {
  const { t } = useTranslations();
  const canManage = useAdminCan('status.manage');
  const [openModal, setOpenModal] = useState<'edit' | null>(null);

  return (
    <>
      <ComponentEditModal component={component} opened={openModal === 'edit'} onClose={() => setOpenModal(null)} />

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faPen,
            label: t('common.button.edit', {}),
            onClick: () => setOpenModal('edit'),
            canAccess: canManage,
          },
        ]}
      >
        {({ items, openMenu }) => (
          <TableRow
            onContextMenu={(e) => {
              e.preventDefault();
              openMenu(e.clientX, e.clientY);
            }}
          >
            <TableData>
              {component.nodeUuid ? (
                <TableLink to={`/admin/nodes/${component.nodeUuid}`}>{component.label}</TableLink>
              ) : (
                component.label
              )}
            </TableData>
            <TableData>{t(KIND_KEYS[component.kind], {})}</TableData>
            <TableData>
              <Badge color={componentStatusColorMapping[component.status]} variant='light'>
                {componentStatusLabelMapping[component.status]()}
              </Badge>
            </TableData>
            <TableData>
              <BooleanText value={component.published} />
            </TableData>
            <TableData>{component.sort}</TableData>
            <TableData>
              {component.checked ? (
                <FormattedTimestamp timestamp={component.checked} />
              ) : (
                <Text size='sm' c='dimmed'>
                  {t('common.na', {})}
                </Text>
              )}
            </TableData>

            <ContextMenuToggle items={items} openMenu={openMenu} />
          </TableRow>
        )}
      </ContextMenu>
    </>
  );
}
