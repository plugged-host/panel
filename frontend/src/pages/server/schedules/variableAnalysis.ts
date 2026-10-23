import { z } from 'zod';
import {
  ServerScheduleCondition,
  serverScheduleSchema,
  serverScheduleStepActionSchema,
  serverScheduleStepSchema,
} from '@/lib/schemas/server/schedules.ts';

type Action = z.infer<typeof serverScheduleStepActionSchema>;
type Step = z.infer<typeof serverScheduleStepSchema>;

export type VariableIssueKind = 'never' | 'unavailable' | 'conditional';

export interface VariableIssue {
  variable: string;
  kind: VariableIssueKind;
}

interface VariableWrite {
  variable: string;
  definite: boolean;
}

interface VariableRead {
  variable: string;
  inCondition: boolean;
}

interface FlowState {
  definite: Set<string>;
  possible: Set<string>;
}

const outputKeys = new Set(['outputInto', 'outputStatusInto', 'outputBodyInto']);

export function triggerWrites(triggers: z.infer<typeof serverScheduleSchema>['triggers']): VariableWrite[] {
  const variables = new Set<string>();
  for (const trigger of triggers) {
    if (trigger.type === 'console_line' && trigger.outputInto?.variable) {
      variables.add(trigger.outputInto.variable);
    }
  }

  return [...variables].map((variable) => ({
    variable,
    definite: triggers.every((trigger) => trigger.type === 'console_line' && trigger.outputInto?.variable === variable),
  }));
}

export function stepWrites(action: Action): VariableWrite[] {
  switch (action.type) {
    case 'format':
      return [{ variable: action.outputInto.variable, definite: true }];
    case 'match_regex':
      return action.outputInto.flatMap((output) => (output ? [{ variable: output.variable, definite: true }] : []));
    case 'wait_for_console_line':
    case 'create_backup':
    case 'create_database_backup':
      return action.outputInto ? [{ variable: action.outputInto.variable, definite: !action.ignoreFailure }] : [];
    case 'http_request':
      return [action.outputStatusInto, action.outputBodyInto].flatMap((output) =>
        output ? [{ variable: output.variable, definite: !action.ignoreFailure }] : [],
      );
    default:
      return [];
  }
}

function conditionReads(condition: ServerScheduleCondition): VariableRead[] {
  switch (condition.type) {
    case 'and':
    case 'or':
      return condition.conditions.flatMap(conditionReads);
    case 'not':
      return conditionReads(condition.condition);
    case 'variable_exists':
      return [{ variable: condition.variable.variable, inCondition: true }];
    case 'variable_equals':
    case 'variable_contains':
    case 'variable_starts_with':
    case 'variable_ends_with':
      return [condition.variable, ...collectDynamicReads(condition)].map(({ variable }) => ({
        variable,
        inCondition: true,
      }));
    default:
      return [];
  }
}

function collectDynamicReads(value: unknown, key?: string): { variable: string }[] {
  if (!value || typeof value !== 'object' || (key && outputKeys.has(key))) return [];
  if (Array.isArray(value)) return value.flatMap((item) => collectDynamicReads(item));

  const entries = Object.entries(value);
  if (entries.length === 1 && entries[0][0] === 'variable' && typeof entries[0][1] === 'string') {
    return [{ variable: entries[0][1] }];
  }

  return entries.flatMap(([childKey, child]) =>
    childKey === 'variable' || childKey === 'condition' ? [] : collectDynamicReads(child, childKey),
  );
}

// mirrors wings: `{{` up to the next `}}`, name trimmed, unclosed placeholders are left alone
export function formatPlaceholders(format: string): string[] {
  return [...format.matchAll(/\{\{([\s\S]*?)\}\}/g)].map((match) => match[1].trim()).filter(Boolean);
}

export function stepReads(action: Action): VariableRead[] {
  switch (action.type) {
    case 'if':
    case 'else_if':
    case 'ensure':
      return conditionReads(action.condition);
    case 'format':
      return formatPlaceholders(action.format).map((variable) => ({ variable, inCondition: false }));
    default:
      return collectDynamicReads(action).map(({ variable }) => ({ variable, inCondition: false }));
  }
}

function conditionGuards(condition: ServerScheduleCondition): string[] {
  switch (condition.type) {
    case 'and':
      return condition.conditions.flatMap(conditionGuards);
    case 'variable_exists':
    case 'variable_equals':
    case 'variable_contains':
    case 'variable_starts_with':
    case 'variable_ends_with':
      return [condition.variable.variable];
    default:
      return [];
  }
}

function cloneState(state: FlowState): FlowState {
  return { definite: new Set(state.definite), possible: new Set(state.possible) };
}

function narrowState(state: FlowState | null, guards: string[]): FlowState | null {
  if (!state) return null;

  const next = cloneState(state);
  for (const variable of guards) {
    next.definite.add(variable);
    next.possible.add(variable);
  }

  return next;
}

function joinStates(states: (FlowState | null)[]): FlowState | null {
  const reachable = states.filter((state): state is FlowState => state !== null);
  if (reachable.length === 0) return null;

  return {
    definite: new Set([...reachable[0].definite].filter((v) => reachable.every((s) => s.definite.has(v)))),
    possible: new Set(reachable.flatMap((s) => [...s.possible])),
  };
}

function checkReads(reads: VariableRead[], state: FlowState, written: Set<string>): VariableIssue[] {
  const issues = new Map<string, VariableIssueKind>();

  for (const { variable, inCondition } of reads) {
    if (!variable || issues.has(variable)) continue;

    if (!written.has(variable)) issues.set(variable, 'never');
    else if (!state.possible.has(variable)) issues.set(variable, 'unavailable');
    else if (!inCondition && !state.definite.has(variable)) issues.set(variable, 'conditional');
  }

  return [...issues].map(([variable, kind]) => ({ variable, kind }));
}

export function analyzeScheduleVariables(
  schedule: Pick<z.infer<typeof serverScheduleSchema>, 'triggers' | 'condition'>,
  steps: Pick<Step, 'uuid' | 'order' | 'action'>[],
): { steps: Map<string, VariableIssue[]>; condition: VariableIssue[] } {
  const initialWrites = triggerWrites(schedule.triggers);
  const written = new Set([
    ...initialWrites.map((write) => write.variable),
    ...steps.flatMap((step) => stepWrites(step.action).map((write) => write.variable)),
  ]);
  const initial: FlowState = {
    definite: new Set(initialWrites.filter((write) => write.definite).map((write) => write.variable)),
    possible: new Set(initialWrites.map((write) => write.variable)),
  };

  const result = new Map<string, VariableIssue[]>();
  const frames: { entry: FlowState | null; ends: (FlowState | null)[]; hasElse: boolean }[] = [];
  let state: FlowState | null = cloneState(initial);

  for (const step of [...steps].sort((a, b) => a.order - b.order)) {
    const action = step.action;
    const frame = frames.at(-1);
    const evaluatedIn = action.type === 'else_if' && frame ? frame.entry : state;

    if (evaluatedIn) {
      const issues = checkReads(stepReads(action), evaluatedIn, written);
      if (issues.length > 0) result.set(step.uuid, issues);
    }

    switch (action.type) {
      case 'if':
        frames.push({ entry: state, ends: [], hasElse: false });
        state = narrowState(state, conditionGuards(action.condition));
        break;
      case 'else_if':
        if (!frame) break;
        frame.ends.push(state);
        state = narrowState(frame.entry, conditionGuards(action.condition));
        break;
      case 'else':
        if (!frame) break;
        frame.ends.push(state);
        frame.hasElse = true;
        state = frame.entry && cloneState(frame.entry);
        break;
      case 'end_if':
        if (!frame) break;
        frames.pop();
        state = joinStates([...frame.ends, state, ...(frame.hasElse ? [] : [frame.entry])]);
        break;
      case 'ensure':
        state = narrowState(state, conditionGuards(action.condition));
        break;
      case 'exit':
        state = null;
        break;
      default:
        if (!state) break;
        for (const write of stepWrites(action)) {
          state.possible.add(write.variable);
          if (write.definite) state.definite.add(write.variable);
        }
    }
  }

  return { steps: result, condition: checkReads(conditionReads(schedule.condition), initial, written) };
}
