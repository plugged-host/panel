import { faCheck, faExclamation } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ReactNode } from 'react';
import { makeComponentHookable } from 'shared';
import ThemeIcon from '@/elements/data-display/ThemeIcon.tsx';
import Group from '@/elements/layout/Group.tsx';
import Text from '@/elements/typography/Text.tsx';

function StatusLine({ status, children }: { status: 'ok' | 'warning'; children: ReactNode }) {
  return (
    <Group gap='xs' wrap='nowrap' align='center'>
      <ThemeIcon size='sm' radius='xl' variant='light' color={status === 'ok' ? 'green' : 'yellow'}>
        <FontAwesomeIcon icon={status === 'ok' ? faCheck : faExclamation} size='xs' />
      </ThemeIcon>
      <Text component='div' size='sm'>
        {children}
      </Text>
    </Group>
  );
}

export default makeComponentHookable(StatusLine);
