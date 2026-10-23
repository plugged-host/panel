import { faCircleUp } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import ContextMenu from '@/elements/overlays/ContextMenu.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminNodeUpdateInformationSchema } from '@/lib/schemas/admin/system.ts';
import NodeUpgradeModal from '@/pages/admin/nodes/modals/NodeUpgradeModal.tsx';
import NodeRow from '@/pages/admin/nodes/NodeRow.tsx';
import { useNodeOnlineWait } from '@/plugins/nodes/useNodeOnlineWait.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useUpgradeRestart } from '@/plugins/useUpgradeRestart.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function OutdatedNodeRow({
  outdatedNode,
}: {
  outdatedNode: z.infer<typeof adminNodeUpdateInformationSchema>;
}) {
  const { t } = useTranslations();
  const canUpgrade = useAdminCan('nodes.upgrade');

  const [openModal, setOpenModal] = useState<'upgrade' | null>(null);
  const onlineWait = useNodeOnlineWait();
  const upgradeRestart = useUpgradeRestart(onlineWait, [
    queryKeys.admin.nodes.systemOverview(outdatedNode.node.uuid),
    queryKeys.admin.nodes.systemDirect(outdatedNode.node.uuid),
    queryKeys.admin.nodes.upgradeStatus(outdatedNode.node.uuid),
    queryKeys.admin.updates.nodes(),
  ]);

  return (
    <>
      {canUpgrade && (
        <NodeUpgradeModal
          node={outdatedNode.node}
          onScheduled={(previousVersion) => upgradeRestart.start(outdatedNode.node.uuid, previousVersion)}
          opened={openModal === 'upgrade'}
          onClose={() => setOpenModal(null)}
        />
      )}

      <ContextMenu
        items={[
          {
            type: 'action',
            icon: faCircleUp,
            label: t('common.button.upgrade', {}),
            onClick: () => setOpenModal('upgrade'),
            disabled: upgradeRestart.restarting,
            canAccess: canUpgrade && outdatedNode.upgradable,
          },
        ]}
      >
        {(props) => (
          <NodeRow
            node={outdatedNode.node}
            upgradeStatus={outdatedNode}
            contextMenuProps={props}
            nameSection={
              upgradeRestart.restarting ? (
                <Badge variant='light' loading>
                  {t('common.badge.restarting', {})}
                </Badge>
              ) : (
                outdatedNode.latestVersion && (
                  <Badge color='yellow' variant='light'>
                    {t('pages.admin.home.tabs.updates.page.badge.latestVersion', {
                      version: outdatedNode.latestVersion,
                    })}
                  </Badge>
                )
              )
            }
          />
        )}
      </ContextMenu>
    </>
  );
}
