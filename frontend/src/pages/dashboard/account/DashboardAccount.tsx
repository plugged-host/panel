import { faEnvelopeCircleCheck, faLock, faShieldAlt } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import getNoprofitAccount from '@/api/me/account/getNoprofitAccount.ts';
import resendEmailVerification from '@/api/me/account/resendEmailVerification.ts';
import Button from '@/elements/buttons/Button.tsx';
import AccountContentContainer from '@/elements/containers/AccountContentContainer.tsx';
import ExtensionSlot from '@/elements/ExtensionSlot.tsx';
import Alert from '@/elements/feedback/Alert.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { noprofitAccountResponseSchema } from '@/lib/schemas/noprofit.ts';
import { useAuth } from '@/providers/AuthProvider.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import AccountContainer from './AccountContainer.tsx';
import AvatarContainer from './AvatarContainer.tsx';
import BadgesContainer from './BadgesContainer.tsx'; // plugged.host
import EmailContainer from './EmailContainer.tsx';
import NoprofitContainer from './NoprofitContainer.tsx';
import PasswordContainer from './PasswordContainer.tsx';
import PasswordLoginContainer from './PasswordLoginContainer.tsx';
import PreferencesContainer from './PreferencesContainer.tsx';
import StandingContainer from './StandingContainer.tsx'; // plugged.host
import TwoFactorContainer from './TwoFactorContainer.tsx';

export interface AccountCardProps {
  requireTwoFactorActivation?: boolean;
}

export default function DashboardAccount() {
  const { t } = useTranslations();
  const { user } = useAuth();
  const { addToast } = useToast();

  const [resending, setResending] = useState(false);
  // plugged.host: a noprofit.space account's email, password and two-factor live there.
  // undefined while loading, so the cards do not swap once it arrives.
  const [noprofit, setNoprofit] = useState<z.infer<typeof noprofitAccountResponseSchema> | null>();

  useEffect(() => {
    getNoprofitAccount()
      .then(setNoprofit)
      .catch(() => setNoprofit(null));
  }, []);

  const noprofitAccount = noprofit?.account ?? null;

  const requireTwoFactorActivation = Boolean(user?.requireTwoFactor && !user?.twoFactorSatisfied);
  const requireEmailVerification = Boolean(user?.requireEmailVerification && !user?.emailVerified);
  const frozen = Boolean(user?.frozen);

  const doResendVerification = () => {
    setResending(true);

    resendEmailVerification()
      .then(({ email }) => addToast(t('pages.account.account.alert.verifyEmail.sent', { email }), 'success'))
      .catch((msg) => addToast(httpErrorToHuman(msg), 'error'))
      .finally(() => setResending(false));
  };

  return (
    <AccountContentContainer
      title={t('pages.account.account.title', {})}
      registry={window.extensionContext.extensionRegistry.pages.dashboard.account.container}
    >
      {frozen && (
        <Alert
          icon={<FontAwesomeIcon icon={faLock} />}
          title={t('pages.account.account.alert.frozen.title', {})}
          color='red'
          mb='md'
        >
          {t('pages.account.account.alert.frozen.description', {})}
        </Alert>
      )}

      {requireEmailVerification && !frozen && (
        <Alert
          icon={<FontAwesomeIcon icon={faEnvelopeCircleCheck} />}
          title={t('pages.account.account.alert.verifyEmail.title', {})}
          color='red'
          mb='md'
        >
          <Stack align='flex-start'>
            {t('pages.account.account.alert.verifyEmail.description', { email: user?.email ?? '' }).md()}
            <Button size='xs' loading={resending} onClick={doResendVerification}>
              {t('pages.account.account.alert.verifyEmail.button.resend', {})}
            </Button>
          </Stack>
        </Alert>
      )}

      {requireTwoFactorActivation && !frozen && (
        <Alert
          icon={<FontAwesomeIcon icon={faShieldAlt} />}
          title={t('pages.account.account.alert.requireTwoFactor.title', {})}
          color='red'
          mb='md'
        >
          {t('pages.account.account.alert.requireTwoFactor.description', {})}
        </Alert>
      )}

      <div
        className={`grid grid-cols-1 md:grid-cols-3 gap-4${
          frozen ? ' pointer-events-none select-none blur-sm opacity-60' : ''
        }`}
        aria-disabled={frozen}
      >
        <ExtensionSlot
          components={
            window.extensionContext.extensionRegistry.pages.dashboard.account.accountContainers.prependedComponents
          }
          name='account-accountContainer-prepended'
          props={{ requireTwoFactorActivation }}
        />

        {noprofit === undefined ? null : noprofitAccount ? (
          <NoprofitContainer account={noprofitAccount} accountsUrl={noprofit?.accountsUrl ?? null} />
        ) : (
          <>
            <PasswordContainer requireTwoFactorActivation={requireTwoFactorActivation} />
            <EmailContainer requireTwoFactorActivation={requireTwoFactorActivation} />
            <TwoFactorContainer />
            <PasswordLoginContainer requireTwoFactorActivation={requireTwoFactorActivation} />
          </>
        )}
        {/* plugged.host: the account's standing, once it has suspensions (docs/features/standing.md) */}
        <StandingContainer />
        <AccountContainer requireTwoFactorActivation={requireTwoFactorActivation} />
        <PreferencesContainer requireTwoFactorActivation={requireTwoFactorActivation} />
        <AvatarContainer requireTwoFactorActivation={requireTwoFactorActivation} />
        {/* plugged.host: profile badges (docs/features/promos.md#profile-badges) */}
        <BadgesContainer />

        <ExtensionSlot
          components={
            window.extensionContext.extensionRegistry.pages.dashboard.account.accountContainers.appendedComponents
          }
          name='account-accountContainer-appended'
          props={{ requireTwoFactorActivation }}
        />
      </div>
    </AccountContentContainer>
  );
}
