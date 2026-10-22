import { faCog, IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { MantineColor, Popover } from '@mantine/core';
import classNames from 'classnames';
import { ReactNode } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import CopyOnClick from '@/elements/CopyOnClick.tsx';
import Card from '@/elements/data-display/Card.tsx';
import ScrollingText from '@/elements/ScrollingText.tsx';
import RedactedText from '@/elements/typography/RedactedText.tsx';
import { usageColor, usagePercent } from '@/lib/format/usage.ts';

/*
 * plugged.host: the Pelican console's stat block ("fi-small-stat-block") on Theme v2's card: the
 * label in gray-400 over the value in white, no icon tile (Theme v2 drops glyph boxes), and the
 * usage as Theme v2's thin bar under it ("phv2-server-track"). `icon` is still accepted, for the
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
}) {
  const displayValue = redact ? <RedactedText value={value} /> : value;
  const color = usageColor(progress, total);
  const percent = usagePercent(progress, total);

  const valueContent = (
    <>
      {displayValue} {limit && <span className='ph-stat-limit'>/ {limit}</span>}
    </>
  );

  return (
    <Card className={classNames('ph-stat', className)} style={{ order }} p={0}>
      <div className='ph-stat-head'>
        <span className='ph-stat-label'>{label}</span>
        {details && <span className='ph-stat-details'>{details}</span>}
        {popover && (
          <Popover position='bottom' withArrow shadow='md'>
            <Popover.Target>
              <Button variant='transparent' size='compact-xs' className='ph-stat-popover' aria-label={label}>
                <FontAwesomeIcon icon={popoverIcon} />
              </Button>
            </Popover.Target>
            <Popover.Dropdown>{popover}</Popover.Dropdown>
          </Popover>
        )}
      </div>
      <span
        className='ph-stat-value'
        style={valueColor ? { color: `var(--mantine-color-${valueColor}-text)` } : undefined}
      >
        {copyOnClick ? (
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
          className='ph-stat-track'
          role='progressbar'
          aria-label={label}
          aria-valuenow={Math.round(percent)}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span
            className='ph-stat-bar'
            style={{ width: `${percent}%`, backgroundColor: color === 'red' ? 'var(--mantine-color-red-4)' : color }}
          />
        </div>
      )}
    </Card>
  );
}
