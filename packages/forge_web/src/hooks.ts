import { useCallback, useEffect, useState } from 'react';

import { useForge } from './context';

export interface Async<T> {
  /** Dernière valeur chargée (conservée pendant un rechargement). */
  data: T | undefined;
  error: unknown;
  reload: () => void;
}

/**
 * Charge une valeur et la recharge quand `deps` changent ou après chaque
 * écriture (`dataVersion`). Une réponse arrivée après un nouveau chargement
 * est ignorée.
 */
export function useAsync<T>(load: () => Promise<T>, deps: readonly unknown[]): Async<T> {
  const { dataVersion } = useForge();
  const [state, setState] = useState<{ data?: T; error?: unknown }>({});
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let current = true;
    load().then(
      (data) => current && setState({ data }),
      (error: unknown) => current && setState({ error }),
    );
    return () => {
      current = false;
    };
    // `load` change à chaque rendu : `deps` décrit ce dont il dépend.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, dataVersion, attempt]);
  const reload = useCallback(() => setAttempt((n) => n + 1), []);
  return { data: state.data, error: state.error, reload };
}

/** Intitulé d'un enregistrement référencé (`#id` en attendant). */
export function useRecordTitle(table: string, id: number): string {
  const { titles } = useForge();
  const [title, setTitle] = useState(`#${id}`);
  useEffect(() => {
    let current = true;
    void titles.title(table, id).then((t) => current && setTitle(t));
    return () => {
      current = false;
    };
  }, [titles, table, id]);
  return title;
}
