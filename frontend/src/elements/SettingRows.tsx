// plugged.host: the label/value rows of the server settings sections, Theme v2's ".phset-*" and
// ".phdb-*" on the Pelican panel (docs/features/server-settings.md).
import { faArrowUpRightFromSquare, faCheck, faCopy } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ActionIcon as MantineActionIcon } from '@mantine/core';
import classNames from 'classnames';
import { ReactNode, useEffect, useState } from 'react';
import ActionIcon from '@/elements/buttons/ActionIcon.tsx';
import Tooltip from '@/elements/overlays/Tooltip.tsx';
import { handleRawCopyToClipboard } from '@/lib/clipboard/copy.ts';

export function SettingRows({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={classNames('theme-setting-rows', className)}>{children}</div>;
}

export function SettingRow({
  label,
  children,
  hint,
  mono,
  wrap,
  muted,
  actions,
}: {
  label: string;
  children: ReactNode;
  hint?: ReactNode;
  mono?: boolean;
  wrap?: boolean;
  muted?: boolean;
  actions?: ReactNode;
}) {
  return (
    <div className='theme-setting-row'>
      <span className='theme-setting-label'>{label}</span>
      {/* a hint always wraps: the one-line value would clip it */}
      <div
        className={classNames(
          'theme-setting-value',
          (wrap || !!hint) && 'theme-setting-value-wrap',
          muted && 'theme-setting-value-muted',
        )}
      >
        {mono ? <span className='font-mono'>{children}</span> : children}
        {hint ? <span className='theme-setting-hint'>{hint}</span> : null}
      </div>
      {actions && <div className='theme-setting-actions'>{actions}</div>}
    </div>
  );
}

// Pelican's copy button: the icon turns into a green check for 2 s, without a toast.
export function CopyActionIcon({ content, label }: { content: string; label: string }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;

    const timer = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timer);
  }, [copied]);

  return (
    <Tooltip label={label}>
      <ActionIcon
        variant='subtle'
        color='gray'
        size={32}
        aria-label={label}
        onClick={() => {
          handleRawCopyToClipboard(content);
          setCopied(true);
        }}
      >
        <FontAwesomeIcon icon={copied ? faCheck : faCopy} className={copied ? 'theme-setting-copied' : undefined} />
      </ActionIcon>
    </Tooltip>
  );
}

// an icon link in a row, e.g. "Launch in SFTP Client"
export function LinkActionIcon({ href, label }: { href: string; label: string }) {
  return (
    <Tooltip label={label}>
      <MantineActionIcon component='a' href={href} variant='subtle' color='gray' size={32} aria-label={label}>
        <FontAwesomeIcon icon={faArrowUpRightFromSquare} />
      </MantineActionIcon>
    </Tooltip>
  );
}
