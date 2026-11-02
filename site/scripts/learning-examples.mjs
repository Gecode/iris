import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { strictEqual, ok } from 'node:assert';
import { repository } from './markdown-sources.mjs';

const readingIds = id => [`${id}-spec`, `${id}-input`, id];
export const learningPages = {
  'tutorial/index.html': [],
  'tutorial/dimacs/index.html': [...readingIds('dimacs-header'), ...readingIds('dimacs-clauses'), ...readingIds('dimacs-nonempty'), 'dimacs-empty-input', 'dimacs-empty', ...readingIds('dimacs-library')],
  'tutorial/tsplib/index.html': [...readingIds('tsplib-headers'), ...readingIds('tsplib-coordinates'), ...readingIds('tsplib-library')],
  'tutorial/job-shop/index.html': [...readingIds('jobshop-records'), ...readingIds('jobshop-checks'), 'jobshop-invalid-input', 'jobshop-invalid', ...readingIds('jobshop-library')],
  'guides/index.html': [],
  'guides/patterns/index.html': [...readingIds('patterns'), 'patterns-invalid-input', 'patterns-invalid'],
  'guides/sections/index.html': [...readingIds('tsplib-coordinates'), 'sections-repeated-input', 'sections-repeated'],
  'guides/records-and-checks/index.html': [...readingIds('jobshop-checks'), 'jobshop-invalid-input', 'jobshop-invalid'],
  'guides/instances/index.html': ['instances-spec', 'instances-input', 'instances-list', 'instances-catalog', 'instances-number', 'instances-name', 'instances-unselected', 'instances-rejected'],
  'guides/parameters/index.html': [...readingIds('parameters'), 'parameters-default', 'parameters-type'],
  'guides/errors/index.html': ['jobshop-checks-spec', 'jobshop-invalid-input', 'jobshop-invalid', 'jobshop-error-json'],
  'guides/performance/index.html': [...readingIds('empty-rows'), 'empty-rows-budget'],
};

// Source files and commands share the documentation collector's CLI and markers.
export async function collectLearningExamples({ tools, execute, command }) {
  const examples = {};
  const addSource = async (id, path, language) => {
    examples[id] = { code: (await readFile(resolve(repository, path), 'utf8')).replace(/\n$/, ''), language };
  };
  const addCommand = (id, args, errorCode) => {
    const result = execute(tools.iris, args, errorCode ? 1 : 0, Boolean(errorCode));
    if (errorCode) {
      strictEqual(result.stdout, '', `${id}: failed read must not print partial data`);
      ok(result.stderr, `${id}: missing diagnostic`);
      const parameters = {};
      for (let i = 0; i < args.length; i++) {
        if (args[i] !== '--param') continue;
        const assignment = args[++i];
        const separator = assignment.indexOf('=');
        parameters[assignment.slice(0, separator)] = assignment.slice(separator + 1);
      }
      const driver = execute(tools.driver, ['read', args[1], args[2], '--params', JSON.stringify(parameters)], 1);
      strictEqual(JSON.parse(driver.stdout).code, errorCode, `${id}: wrong error kind`);
    }
    examples[id] = { code: command(args), language: 'sh', output: errorCode ? result.stderr : result.stdout };
  };
  async function addReading(id, spec, input, params = []) {
    execute(tools.iris, ['check', spec], 0);
    await addSource(`${id}-spec`, spec, 'iris');
    await addSource(`${id}-input`, input, 'text');
    addCommand(id, ['read', spec, input, '--dzn', ...params]);
  }
  const tutorial = 'site/examples/tutorial';
  const guides = 'site/examples/guides';
  execute(tools.iris, ['fmt', '--check', 'site/examples/'], 0);
  await addReading('dimacs-header', `${tutorial}/dimacs-header.iris`, `${tutorial}/dimacs-header.txt`);
  await addReading('dimacs-clauses', `${tutorial}/dimacs-clauses.iris`, `${tutorial}/dimacs-clauses.txt`);
  await addReading('dimacs-nonempty', `${tutorial}/dimacs-nonempty.iris`, `${tutorial}/dimacs-clauses.txt`);
  await addSource('dimacs-empty-input', `${tutorial}/dimacs-empty.txt`, 'text');
  addCommand('dimacs-empty', ['read', `${tutorial}/dimacs-nonempty.iris`, `${tutorial}/dimacs-empty.txt`, '--dzn'], 'E-CHECK-FAILED');
  await addReading('dimacs-library', 'formats/dimacs-cnf/dimacs-cnf.iris', 'formats/dimacs-cnf/cases/three-variables/input.txt');
  await addReading('tsplib-headers', `${tutorial}/tsplib-headers.iris`, `${tutorial}/tsplib-headers.txt`);
  await addReading('tsplib-coordinates', `${tutorial}/tsplib-coordinates.iris`, `${tutorial}/tsplib-coordinates.txt`);
  await addReading('tsplib-library', 'formats/tsplib/tsplib.iris', 'formats/tsplib/cases/euc-2d-five/input.txt');
  await addReading('jobshop-records', `${tutorial}/jobshop-records.iris`, `${tutorial}/jobshop.txt`);
  await addReading('jobshop-checks', `${tutorial}/jobshop-checks.iris`, `${tutorial}/jobshop.txt`);
  await addSource('jobshop-invalid-input', `${guides}/jobshop-invalid.txt`, 'text');
  addCommand('jobshop-invalid', ['read', `${tutorial}/jobshop-checks.iris`, `${guides}/jobshop-invalid.txt`, '--dzn'], 'E-CHECK-FAILED');
  await addReading('jobshop-library', 'formats/jobshop/jobshop.iris', 'formats/jobshop/cases/or-library/input.txt', ['--param', 'instance=handmade-or']);
  await addReading('patterns', `${guides}/patterns.iris`, `${guides}/patterns.txt`);
  await addSource('patterns-invalid-input', `${guides}/patterns-invalid.txt`, 'text');
  addCommand('patterns-invalid', ['read', `${guides}/patterns.iris`, `${guides}/patterns-invalid.txt`, '--dzn'], 'E-LINE-MATCH');
  await addSource('sections-repeated-input', `${guides}/sections-repeated.txt`, 'text');
  addCommand('sections-repeated', ['read', `${tutorial}/tsplib-coordinates.iris`, `${guides}/sections-repeated.txt`, '--dzn'], 'E-SECTIONS-REPEATED');
  const instances = [`${guides}/instances.iris`, `${guides}/instances.txt`];
  execute(tools.iris, ['check', instances[0]], 0);
  await addSource('instances-spec', instances[0], 'iris');
  await addSource('instances-input', instances[1], 'text');
  addCommand('instances-list', ['list', ...instances]);
  addCommand('instances-catalog', ['list', ...instances, '--dzn']);
  addCommand('instances-number', ['read', ...instances, '--dzn', '--param', 'instance=3']);
  addCommand('instances-name', ['read', ...instances, '--dzn', '--param', 'instance=large']);
  addCommand('instances-unselected', ['read', ...instances, '--dzn'], 'E-INSTANCE-SELECTION');
  addCommand('instances-rejected', ['read', ...instances, '--dzn', '--param', 'instance=broken'], 'E-CHECK-FAILED');
  await addReading('parameters', `${guides}/parameters.iris`, `${guides}/parameters.txt`, ['--param', 'capacity=12']);
  addCommand('parameters-default', ['read', `${guides}/parameters.iris`, `${guides}/parameters.txt`, '--dzn'], 'E-CHECK-FAILED');
  addCommand('parameters-type', ['read', `${guides}/parameters.iris`, `${guides}/parameters.txt`, '--dzn', '--param', 'capacity=12.5'], 'E-PARAM-TYPE');
  const errorArgs = ['read', `${tutorial}/jobshop-checks.iris`, `${guides}/jobshop-invalid.txt`];
  examples['jobshop-error-json'] = { code: command(errorArgs, 'iris-driver'), language: 'sh', output: execute(tools.driver, errorArgs, 1).stdout };
  await addReading('empty-rows', `${guides}/empty-rows.iris`, `${guides}/empty.txt`);
  addCommand('empty-rows-budget', ['read', `${guides}/empty-rows.iris`, `${guides}/empty.txt`, '--dzn', '--param', 'rows=1048577'], 'E-BUDGET');
  return examples;
}
