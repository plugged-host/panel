import Stack from '@/elements/layout/Stack.tsx';
import { useTranslations } from '@/providers/TranslationProvider.tsx';
import { VariableIssue } from './variableAnalysis.ts';

export default function VariableIssueList({ issues }: { issues: VariableIssue[] }) {
  const { t } = useTranslations();

  return (
    <Stack gap={4}>
      {issues.map((issue) => (
        <span key={issue.variable}>
          {t(`pages.server.schedules.steps.warning.variable.${issue.kind}`, { variable: issue.variable })}
        </span>
      ))}
    </Stack>
  );
}
