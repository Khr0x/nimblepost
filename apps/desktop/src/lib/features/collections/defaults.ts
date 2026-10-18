import type { DefaultsSource } from '../../desktop/api';
import type { Config } from '../requests/draft';

export function editableDefaults(value: Config) {
  return value.request === undefined || (!!value.request && typeof value.request === 'object' && !Array.isArray(value.request));
}

export function defaultsProvenance(ancestors: DefaultsSource[], path: string, value: Config) {
  const defaults = value.request;
  const names = (key: string): DefaultsSource['headers'] => (Array.isArray(defaults?.[key]) ? defaults[key] : []).map((row: Config | null) => ({ name: typeof row?.name === 'string' ? row.name : '[invalid name]', disabled: !!row?.disabled }));
  const sources: DefaultsSource[] = [...ancestors, { path, headers: names('headers'), variables: names('variables'), auth: typeof defaults?.auth === 'string' ? defaults.auth : defaults?.auth?.type ?? null }];
  return sources.flatMap((source, index) => {
    const later = sources.slice(index + 1);
    const current = index === sources.length - 1 ? 'local' : 'inherited';
    return [
      ...source.headers.map(row => ({ section: 'Headers', name: row.name, path: source.path,
        state: later.some(scope => scope.headers.some(next => next.name.toLowerCase() === row.name.toLowerCase())) ? 'overridden'
          : source.headers.filter(next => next.name.toLowerCase() === row.name.toLowerCase()).every(next => next.disabled) ? 'suppressed' : row.disabled ? 'disabled' : current })),
      ...source.variables.map(row => ({ section: 'Variables', name: row.name, path: source.path,
        state: row.disabled ? 'disabled' : later.some(scope => scope.variables.some(next => !next.disabled && next.name === row.name)) ? 'overridden' : current })),
      ...(source.auth && source.auth !== 'inherit' ? [{ section: 'Auth', name: source.auth, path: source.path,
        state: later.some(scope => scope.auth && scope.auth !== 'inherit') ? 'overridden' : current }] : []),
    ];
  });
}
