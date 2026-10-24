// plugged.host: one unit of neo's work (a tool or a model call), streamed as NDJSON: `text`,
// `thinking` and `reset` lines while the model writes, `ping` lines while nothing happens, then one
// `result` line. Plain fetch, as axios cannot hand over a response body while it arrives; it sends
// what the axios client sends (the session cookie, the impersonation header).
import { getImpersonatedUser } from '@/api/axios.ts';
import { neoBase } from '@/lib/neo.ts';
import { NeoStreamResult, neoStreamResultSchema } from '@/lib/schemas/user/neo.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface NeoStepHandlers {
  onText: (text: string) => void;
  onThinking: (text: string) => void;
  /** What came so far is void: the model call is being made again. */
  onReset: () => void;
}

// shaped like an axios error, so httpErrorToHuman and getHttpStatus read it
function failed(status: number, data: unknown): Error {
  return Object.assign(new Error(`Request failed with status code ${status}`), { response: { status, data } });
}

export default async (serverUuid: string | null, handlers: NeoStepHandlers): Promise<NeoStreamResult> => {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/x-ndjson',
  };
  const impersonated = getImpersonatedUser();
  if (impersonated) {
    headers['Calagopus-User'] = impersonated;
  }

  const response = await fetch(`${neoBase(serverUuid)}/step`, {
    method: 'POST',
    headers,
    body: '{}',
    credentials: 'same-origin',
  });

  if (!response.ok || !response.body) {
    if (response.status === 401) {
      window.dispatchEvent(new Event('session-expired'));
    }

    let data: unknown = null;
    try {
      data = await response.json();
    } catch {
      // not JSON: the status says enough
    }

    throw failed(response.status, data);
  }

  // set by the result line; a holder, as the line is read inside a callback
  const received: { result: NeoStreamResult | null } = { result: null };

  const handle = (line: string) => {
    const text = line.trim();
    if (!text) return;

    let event: { type?: string; text?: unknown };
    try {
      event = JSON.parse(text);
    } catch {
      return;
    }

    switch (event.type) {
      case 'text':
        if (typeof event.text === 'string') handlers.onText(event.text);
        break;
      case 'thinking':
        if (typeof event.text === 'string') handlers.onThinking(event.text);
        break;
      case 'reset':
        handlers.onReset();
        break;
      case 'result':
        received.result = parseFromApi(neoStreamResultSchema, event);
        break;
      default:
        // ping: keeps proxies from closing a quiet stream
        break;
    }
  };

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    let newline = buffer.indexOf('\n');
    while (newline >= 0) {
      handle(buffer.slice(0, newline));
      buffer = buffer.slice(newline + 1);
      newline = buffer.indexOf('\n');
    }
  }
  handle(buffer + decoder.decode());

  if (!received.result) {
    // the stream broke off before its result: the chat says the connection was lost
    throw new Error('neo step ended without a result');
  }

  return received.result;
};
