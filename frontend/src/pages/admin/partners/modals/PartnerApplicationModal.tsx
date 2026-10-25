// plugged.host: Admin › Partners: an application with what the bot checked, neo's suggestion,
// the decision and the payments.
import { ModalProps } from '@mantine/core';
import { ReactNode } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { Modal, ModalFooter } from '@/elements/modals/Modal.tsx';
import FormattedTimestamp from '@/elements/time/FormattedTimestamp.tsx';
import Code from '@/elements/typography/Code.tsx';
import Text from '@/elements/typography/Text.tsx';
import { AdminPartnerApplication } from '@/lib/schemas/admin/partners.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { partnerGameLabel, partnerProblemLabel, partnerStatusLabelMapping } from '../partnerLabels.ts';

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Group gap='xs' wrap='nowrap' align='flex-start'>
      <Text size='sm' c='dimmed' w={170} className='shrink-0'>
        {label}
      </Text>
      <Text size='sm' component='div'>
        {children}
      </Text>
    </Group>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap={4}>
      <Text size='sm' fw={600}>
        {title}
      </Text>
      {children}
    </Stack>
  );
}

export default function PartnerApplicationModal({
  application,
  ...props
}: ModalProps & { application: AdminPartnerApplication }) {
  const { t } = useTranslations();
  const { facts, assessment } = application;
  const page = 'pages.admin.partners.page.detail';
  const onlineShare = facts.members > 0 ? Math.round((facts.online * 100) / facts.members) : 0;

  return (
    <Modal title={application.guildName} size='lg' {...props}>
      <Stack>
        <Section title={t(`${page}.server`, {})}>
          <Detail label={t(`${page}.guildId`, {})}>
            <Code>{application.guildId}</Code>
          </Detail>
          {application.inviteUrl && (
            <Detail label={t(`${page}.invite`, {})}>
              <a href={application.inviteUrl} target='_blank' rel='noopener noreferrer' dir='ltr'>
                {application.inviteUrl}
              </a>
            </Detail>
          )}
          <Detail label={t(`${page}.members`, {})}>
            {t(`${page}.membersValue`, {
              members: facts.members.toLocaleString(),
              online: facts.online.toLocaleString(),
              share: onlineShare,
            })}
          </Detail>
          {facts.created && (
            <Detail label={t(`${page}.serverCreated`, {})}>
              <FormattedTimestamp timestamp={facts.created} />
            </Detail>
          )}
          <Detail label={t(`${page}.boosts`, {})}>
            {t(`${page}.boostsValue`, { level: facts.boostTier, boosts: facts.boosts })}
          </Detail>
          <Detail label={t(`${page}.verification`, {})}>{facts.verificationLevel}</Detail>
          {facts.features.length > 0 && <Detail label={t(`${page}.features`, {})}>{facts.features.join(', ')}</Detail>}
          <Detail label={t(`${page}.standing`, {})}>
            {t(`pages.admin.partners.enum.standing.${facts.standing}`, {})}
          </Detail>
          <Detail label={t(`${page}.channel`, {})}>
            {facts.channel ? `#${facts.channel.name}` : t(`${page}.channelNone`, {})}
          </Detail>
          <Detail label={t(`${page}.checked`, {})}>
            <FormattedTimestamp timestamp={application.checked} />
          </Detail>
          {application.problems.length > 0 && (
            <Detail label={t(`${page}.problems`, {})}>
              <Stack gap={2}>
                {application.problems.map((problem) => (
                  <Text key={problem} size='sm' c='red'>
                    {partnerProblemLabel(problem)}
                  </Text>
                ))}
              </Stack>
            </Detail>
          )}
          {facts.description && <Detail label={t(`${page}.serverDescription`, {})}>{facts.description}</Detail>}
        </Section>

        <Section title={t(`${page}.application`, {})}>
          {application.user && (
            <Detail label={t(`${page}.applicant`, {})}>
              {application.user.username} · {t(`${page}.accountSince`, {})}{' '}
              <FormattedTimestamp timestamp={application.user.created} />
            </Detail>
          )}
          <Detail label={t(`${page}.discordId`, {})}>
            <Code>{application.discordId}</Code>
          </Detail>
          <Detail label={t(`${page}.game`, {})}>{partnerGameLabel(application.game)}</Detail>
          {application.servers.length > 0 && (
            <Detail label={t(`${page}.servers`, {})}>
              {application.servers.map((server) => server.name).join(', ')}
            </Detail>
          )}
          <Text size='sm' className='whitespace-pre-wrap'>
            {application.description}
          </Text>
        </Section>

        <Section title={t(`${page}.neo`, {})}>
          {assessment ? (
            <>
              <Group gap='xs'>
                <Badge color={assessment.accept ? 'green' : 'red'} variant='light'>
                  {assessment.accept
                    ? t('pages.admin.partners.page.neo.accept', { credits: assessment.credits.toLocaleString() })
                    : t('pages.admin.partners.page.neo.reject', {})}
                </Badge>
                {assessment.suggested !== null && (
                  <Text size='sm' c='dimmed'>
                    {t(`${page}.neoSuggested`, { credits: assessment.suggested.toLocaleString() })}
                  </Text>
                )}
              </Group>
              {assessment.reasons.map((reason) => (
                <Text key={reason} size='sm'>
                  {reason}
                </Text>
              ))}
              <Text size='xs' c='dimmed'>
                {assessment.model ? t(`${page}.neoModel`, { model: assessment.model }) : null}
                {application.assessed && (
                  <>
                    {' '}
                    <FormattedTimestamp timestamp={application.assessed} />
                  </>
                )}
              </Text>
            </>
          ) : (
            <Text size='sm' c='dimmed'>
              {application.assessmentError
                ? t(`pages.admin.partners.enum.assessmentError.${application.assessmentError}`, {})
                : t('pages.admin.partners.page.neo.waiting', {})}
            </Text>
          )}
          <Text size='xs' c='dimmed'>
            {t(`${page}.neoOnlySuggests`, {})}
          </Text>
        </Section>

        <Section title={t(`${page}.decision`, {})}>
          <Detail label={t('common.table.columns.status', {})}>
            {partnerStatusLabelMapping[application.status]()}
          </Detail>
          {application.decided && (
            <Detail label={t(`${page}.decided`, {})}>
              {application.decidedBy ? `${application.decidedBy} · ` : ''}
              <FormattedTimestamp timestamp={application.decided} />
            </Detail>
          )}
          {application.reason && <Detail label={t(`${page}.reason`, {})}>{application.reason}</Detail>}
          {application.approved && (
            <Detail label={t(`${page}.partnerSince`, {})}>
              <FormattedTimestamp timestamp={application.approved} />
            </Detail>
          )}
          {['approved', 'paused', 'suspended'].includes(application.status) && (
            <Detail label={t(`${page}.monthly`, {})}>{application.monthlyCredits.toLocaleString()}</Detail>
          )}
          {application.status === 'approved' && application.nextPayment && (
            <Detail label={t(`${page}.nextPayment`, {})}>
              <FormattedTimestamp timestamp={application.nextPayment} />
            </Detail>
          )}
          {application.suspendAfter && (
            <Detail label={t(`${page}.suspendAfter`, {})}>
              <FormattedTimestamp timestamp={application.suspendAfter} />
            </Detail>
          )}
        </Section>

        {application.payments.length > 0 && (
          <Section title={t(`${page}.payments`, { total: application.paidTotal.toLocaleString() })}>
            {application.payments.map((payment) => (
              <Detail key={payment.period} label={payment.period}>
                {payment.credits.toLocaleString()} · <FormattedTimestamp timestamp={payment.created} />
              </Detail>
            ))}
          </Section>
        )}

        <ModalFooter>
          <Button variant='default' onClick={props.onClose}>
            {t('common.button.close', {})}
          </Button>
        </ModalFooter>
      </Stack>
    </Modal>
  );
}
