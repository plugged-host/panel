import { faCog, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { MantineColor, Popover } from '@mantine/core';
import classNames from 'classnames';
import { ReactNode } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Card from '@/elements/data-display/Card.tsx';
import Spinner from '@/elements/feedback/Spinner.tsx';
import ScrollingText from '@/elements/ScrollingText.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { usageColor, usagePercent } from '@/lib/format/usage.ts';

/*
 * plugged.host: the Pelican console's stat block ("fi-small-stat-block") on Theme v2's card: the
 * label in gray-400 over the value in white, no icon tile (Theme v2 drops glyph boxes), and the
 * usage as Theme v2's thin bar under it ("theme-server-track"). `icon` is still accepted, for the
 * callers and extensions that pass one.
 */
export default function StatCard({
  label,
  value,
  order,
  className,
  copyOnClick,
  popover,
  popoverIcon = faCog,
  limit,
  details,
  progress,
  total,
  valueColor,
  redact,
  loading,
}: {
  icon?: IconDefinition;
  label: string;
  value: string;
  order?: number;
  className?: string;
  copyOnClick?: boolean;
  popover?: ReactNode;
  popoverIcon?: IconDefinition;
  limit?: string | null;
  details?: string | null;
  progress?: number | null;
  total?: number | null;
  valueColor?: MantineColor;
  redact?: boolean;
  loading?: boolean;
}) {
  const displayValue = redact ? <RedactedText value={value} /> : value;
  const shownProgress = loading ? null : progress;
  const color = usageColor(shownProgress, total);
  const percent = usagePercent(shownProgress, total);

  const valueContent = (
    <>
      {displayValue} {limit && <span className='theme-stat-limit'>/ {limit}</span>}
    </>
  );

  return (
    <Card className={classNames('theme-stat', className)} style={{ order }} p={0}>
      <div className='theme-stat-head'>
        <span className='theme-stat-label'>{label}</span>
        {details && !loading && <span className='theme-stat-details'>{details}</span>}
        {popover && (
          <Popover position='bottom' withArrow shadow='md'>
            <Popover.Target>
              <Button variant='transparent' size='compact-xs' className='theme-stat-popover' aria-label={label}>
                <FontAwesomeIcon icon={popoverIcon} />
              </Button>
            </Popover.Target>
            <Popover.Dropdown>{popover}</Popover.Dropdown>
          </Popover>
        )}
      </div>
      <span
        className='theme-stat-value'
        style={valueColor ? { color: `var(--mantine-color-${valueColor}-text)` } : undefined}
      >
        {loading ? (
          <span className='flex h-7 items-center'>
            <Spinner size={18} />
          </span>
        ) : copyOnClick ? (
          <ScrollingText>
            <CopyOnClick content={value} className='text-left block'>
              {valueContent}
            </CopyOnClick>
          </ScrollingText>
        ) : (
          <ScrollingText>{valueContent}</ScrollingText>
        )}
      </span>
      {percent !== null && (
        <div
          className='theme-stat-track'
          role='progressbar'
          aria-label={label}
          aria-valuenow={Math.round(percent)}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span
            className='theme-stat-bar'
            style={{ width: `${percent}%`, backgroundColor: color === 'red' ? 'var(--mantine-color-red-4)' : color }}
          />
        </div>
      )}
    </Card>
  );
}
