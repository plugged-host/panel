import { faCircleUp } from '@fortawesome/free-solid-svg-icons';
import { useState } from 'react';
import { z } from 'zod';
import Badge from '@/elements/data-display/Badge.tsx';
import ContextMenu from '@/elements/overlays/ContextMenu.tsx';
import { queryKeys } from '@/lib/queryKeys.ts';
import { adminDatabaseAgentHostUpdateInformationSchema } from '@/lib/schemas/admin/system.ts';
import DatabaseAgentHostRow from '@/pages/admin/database-agent-hosts/DatabaseAgentHostRow.tsx';
import DatabaseAgentHostUpgradeModal from '@/pages/admin/database-agent-hosts/modals/DatabaseAgentHostUpgradeModal.tsx';
import { useDatabaseAgentHostOnlineWait } from '@/plugins/database-agent-hosts/useDatabaseAgentHostOnlineWait.ts';
import { useAdminCan } from '@/plugins/usePermissions.ts';
import { useUpgradeRestart } from '@/plugins/useUpgradeRestart.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

export default function OutdatedDatabaseAgentHostRow({
  outdatedDatabaseAgentHost,
}: {
  outdatedDatabaseAgentHost: z.infer<typeof adminDatabaseAgentHostUpdateInformationSchema>;
}) {
  const { t } = useTranslations();
  const canUpgrade = useAdminCan('database-agent-hosts.upgrade');

  const [openModal, setOpenModal] = useState<'upgrade' | null>(null);
  const onlineWait = useDatabaseAgentHostOnlineWait();
  const upgradeRestart = useUpgradeRestart(onlineWait, [
    queryKeys.admin.databaseAgentHosts.systemOverview(outdatedDatabaseAgentHost.databaseAgentHost.uuid),
    queryKeys.admin.databaseAgentHosts.upgradeStatus(outdatedDatabaseAgentHost.databaseAgentHost.uuid),
    queryKeys.admin.updates.databaseAgentHosts(),
  ]);

  return (
    <>
      {canUpgrade && (
        <DatabaseAgentHostUpgradeModal
          databaseAgentHost={outdatedDatabaseAgentHost.databaseAgentHost}
          onScheduled={(previousVersion) =>
            upgradeRestart.start(outdatedDatabaseAgentHost.databaseAgentHost.uuid, previousVersion)
          }
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
            canAccess: canUpgrade && outdatedDatabaseAgentHost.upgradable,
          },
        ]}
      >
        {(props) => (
          <DatabaseAgentHostRow
            databaseAgentHost={outdatedDatabaseAgentHost.databaseAgentHost}
            upgradeStatus={outdatedDatabaseAgentHost}
            contextMenuProps={props}
            nameSection={
              upgradeRestart.restarting ? (
                <Badge variant='light' loading>
                  {t('common.badge.restarting', {})}
                </Badge>
              ) : (
                outdatedDatabaseAgentHost.latestVersion && (
                  <Badge color='yellow' variant='light'>
                    {t('pages.admin.home.tabs.updates.page.badge.latestVersion', {
                      version: outdatedDatabaseAgentHost.latestVersion,
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
