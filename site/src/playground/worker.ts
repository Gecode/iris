import { initialize, check, read, format } from '../generated/playground-wasm';
import type { Request, Response, CheckResult, ReadResult, FormatResult } from './protocol';
const send = (response: Response) => self.postMessage(response);
try {
  await initialize();
  self.onmessage = (event: MessageEvent<Request>) => {
    const request = event.data;
    const identity = { generation: request.generation, id: request.id };
    try {
      if (request.kind === 'format') {
        const result: FormatResult = JSON.parse(format(JSON.stringify({ language: '0.1', spec: request.spec, specOrigin: request.specOrigin })));
        send({ kind: 'formatted', ...identity, result });
        return;
      }
      const { language, spec, specOrigin, input, inputOrigin, params } = request.state;
      const result: CheckResult = JSON.parse(check(JSON.stringify({ language, spec, specOrigin })));
      send({ kind: 'checked', ...identity, result });
      if (result.kind !== 'checked') return;
      const output: ReadResult = JSON.parse(read(JSON.stringify({ language, spec, specOrigin, input, inputOrigin, params })));
      send({ kind: 'result', ...identity, result: output });
    } catch (error) { send({ kind: 'failed', message: error instanceof Error ? error.message : String(error) }); }
  };
  send({ kind: 'ready' });
} catch (error) { send({ kind: 'failed', message: error instanceof Error ? error.message : String(error) }); }
