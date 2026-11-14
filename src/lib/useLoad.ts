import { useCallback, useEffect, useEffectEvent, useState } from 'react';
import { errorMessage } from './api';

export type Loadable<T> =
  | { status: 'loading' }
  | { status: 'ready'; data: T }
  | { status: 'error'; error: string };

interface Settled<T> {
  token: string;
  value: Loadable<T>;
}

/**
 * Load data from the core. Reloads when `key` changes or `reload` is called.
 * Results from a superseded load are dropped.
 */
export function useLoad<T>(
  load: () => Promise<T>,
  key: string,
): { state: Loadable<T>; reload: () => void } {
  const [nonce, setNonce] = useState(0);
  const token = `${key}#${nonce}`;
  const [settled, setSettled] = useState<Settled<T> | null>(null);
  const run = useEffectEvent(load);

  useEffect(() => {
    let live = true;
    run().then(
      (data) => {
        if (live) setSettled({ token, value: { status: 'ready', data } });
      },
      (err: unknown) => {
        if (live) setSettled({ token, value: { status: 'error', error: errorMessage(err) } });
      },
    );
    return () => {
      live = false;
    };
  }, [token]);

  const reload = useCallback(() => {
    setNonce((n) => n + 1);
  }, []);

  const state: Loadable<T> = settled?.token === token ? settled.value : { status: 'loading' };
  return { state, reload };
}
