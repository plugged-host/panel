// plugged.host: "Getting started" on the Tokens page: a new account's first-week tasks, each paying
// once; a done task is claimed (checked by the panel), the others link to where they are done
// (docs/features/economy.md#getting-started).
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { httpErrorToHuman } from '@/api/axios.ts';
import claimStarterTask from '@/api/me/economy/claimStarterTask.ts';
import Button from '@/elements/buttons/Button.tsx';
import Badge from '@/elements/data-display/Badge.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import { intlLocale } from '@/lib/languages.ts';
import { queryKeys } from '@/lib/queryKeys.ts';
import { starterTaskSchema, tokenStarterSchema } from '@/lib/schemas/user/economy.ts';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type Task = z.infer<typeof starterTaskSchema>;

const page = 'pages.account.tokens.starter';

// where each task is done; earning is on this page, under the ways to earn
const paths: Record<Task, string | null> = {
  discord: '/account/discord',
  create_server: '/create',
  start_server: '/',
  earn: null,
};

export default function StarterTasks({ starter }: { starter: z.infer<typeof tokenStarterSchema> }) {
  const { t, tItem, language } = useTranslations();
  const { addToast } = useToast();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [claiming, setClaiming] = useState<Task | null>(null);

  const claim = (task: Task) => {
    setClaiming(task);
    claimStarterTask(task)
      .then((amount) => {
        addToast(t(`${page}.toast`, { tokens: tItem('token', amount) }), 'success');
        queryClient.invalidateQueries({ queryKey: queryKeys.user.tokens() });
        queryClient.invalidateQueries({ queryKey: queryKeys.user.balance() });
      })
      .catch((error) => addToast(httpErrorToHuman(error), 'error'))
      .finally(() => setClaiming(null));
  };

  const go = (task: Task) => {
    const path = paths[task];
    if (path) {
      navigate(path);
    } else {
      document.getElementById('earn')?.scrollIntoView({ block: 'start' });
    }
  };

  return (
    <TitleCard
      title={t(`${page}.title`, {})}
      description={t(`${page}.description`, {
        date: new Intl.DateTimeFormat(intlLocale(language), { dateStyle: 'medium' }).format(starter.ends),
      })}
    >
      <div className='theme-earn-rows flex flex-col'>
        {starter.tasks.map((task) => (
          <div key={task.task} className='theme-earn-row'>
            <div className='flex min-w-0 flex-col gap-0.5'>
              <span className='theme-eco-title'>{t(`${page}.task.${task.task}`, {})}</span>
              <span className='theme-eco-muted'>{t(`${page}.reward`, { tokens: tItem('token', starter.reward) })}</span>
            </div>

            <div className='theme-earn-action'>
              {task.claimed ? (
                <Badge color='gray' size='sm'>
                  {t(`${page}.claimed`, {})}
                </Badge>
              ) : task.done ? (
                <Button
                  variant='default'
                  fullWidth
                  loading={claiming === task.task}
                  disabled={claiming !== null && claiming !== task.task}
                  onClick={() => claim(task.task)}
                >
                  {t(`${page}.claim`, {})}
                </Button>
              ) : (
                <Button variant='default' fullWidth onClick={() => go(task.task)}>
                  {t(`${page}.go.${task.task}`, {})}
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>
    </TitleCard>
  );
}
