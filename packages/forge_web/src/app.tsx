import 'dayjs/locale/fr';

import { MantineProvider } from '@mantine/core';
import { DatesProvider } from '@mantine/dates';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import {
  createBrowserRouter,
  createMemoryRouter,
  Navigate,
  RouterProvider,
  type RouteObject,
  useLocation,
  useParams,
  useSearchParams,
} from 'react-router';

import { ForgeClient } from './api/client';
import { can, type Forge, ForgeContext, useForge } from './context';
import type { ForgeCustomization } from './customization';
import { stringsFor } from './i18n';
import { paths } from './paths';
import { type AppSchema, findTable } from './schema';
import { buildTheme } from './theme';
import { TitleCache } from './titles';
import { AccountPage } from './ui/AccountPage';
import { Loading, NotFound } from './ui/common';
import { DetailPage } from './ui/DetailPage';
import { FormPage } from './ui/FormPage';
import { HomePage } from './ui/HomePage';
import { LoginPage } from './ui/LoginPage';
import { ParametersPage } from './ui/ParametersPage';
import { Shell } from './ui/Shell';
import { TablePage } from './ui/TablePage';
import { UserFormPage, UsersPage } from './ui/UsersPage';
import { ValueFormat } from './values';

/** Clé de la langue choisie dans le stockage du client. */
const LOCALE_KEY = 'forge.locale';

/** Pages connectées : redirige vers la connexion sans session. */
function Protected() {
  const { user } = useForge();
  const location = useLocation();
  if (!user) {
    const from = location.pathname === '/' ? '' : `?from=${encodeURIComponent(location.pathname + location.search)}`;
    return <Navigate to={`${paths.login}${from}`} replace />;
  }
  return <Shell />;
}

function Login() {
  const { user } = useForge();
  const [params] = useSearchParams();
  return user ? <Navigate to={params.get('from') ?? paths.home} replace /> : <LoginPage />;
}

/** Table de l'URL, si elle existe et est lisible. */
function useTable() {
  const forge = useForge();
  const { table: name } = useParams();
  const table = name ? findTable(forge.schema, name) : undefined;
  return table && can(forge, table, 'read') ? table : undefined;
}

function useId() {
  const { id } = useParams();
  const parsed = Number(id);
  return Number.isInteger(parsed) ? parsed : undefined;
}

function TableRoute() {
  const table = useTable();
  // Clé : une autre table ou d'autres filtres d'URL repartent de zéro.
  const { search } = useLocation();
  return table ? <TablePage key={table.name + search} table={table} /> : <NotFound />;
}

function DetailRoute() {
  const table = useTable();
  const id = useId();
  return table && id !== undefined ? <DetailPage key={`${table.name}/${id}`} table={table} id={id} /> : <NotFound />;
}

function FormRoute({ creating }: { creating: boolean }) {
  const table = useTable();
  const id = useId();
  if (!table || (!creating && id === undefined)) return <NotFound />;
  return <FormPage key={`${table.name}/${id ?? 'new'}`} table={table} id={creating ? undefined : id} />;
}

function CustomPageRoute() {
  const forge = useForge();
  const { page } = useParams();
  const roles = forge.user?.roles ?? [];
  const found = forge.customization.pages?.find(
    (p) => p.path === page && (!p.roles || p.roles.some((r) => roles.includes(r))),
  );
  return found ? <found.component /> : <NotFound />;
}

function UserRoute() {
  const id = useId();
  return id === undefined ? <NotFound /> : <UserFormPage key={id} id={id} />;
}

const routes: RouteObject[] = [
  { path: paths.login, element: <Login /> },
  {
    element: <Protected />,
    children: [
      { index: true, element: <HomePage /> },
      { path: 'data/:table', element: <TableRoute /> },
      { path: 'data/:table/new', element: <FormRoute creating /> },
      { path: 'data/:table/:id', element: <DetailRoute /> },
      { path: 'data/:table/:id/edit', element: <FormRoute creating={false} /> },
      { path: 'pages/:page', element: <CustomPageRoute /> },
      { path: paths.account, element: <AccountPage /> },
      { path: paths.parameters, element: <ParametersPage /> },
      { path: paths.users, element: <UsersPage /> },
      { path: paths.newUser, element: <UserFormPage /> },
      { path: 'settings/users/:id', element: <UserRoute /> },
      { path: '*', element: <NotFound /> },
    ],
  },
];

export interface ForgeAppProps {
  schema: AppSchema;
  /** Client de l'API (par défaut : l'origine de la page ou `VITE_FORGE_API_URL`). */
  client?: ForgeClient;
  customization?: ForgeCustomization;
  /** Adresse de départ ; avec elle, la navigation reste en mémoire (tests). */
  initialPath?: string;
}

/** Application complète décrite par `schema`. */
export function ForgeApp({ schema, client: given, customization = {}, initialPath }: ForgeAppProps) {
  const client = useMemo(() => given ?? new ForgeClient(), [given]);
  const user = useSyncExternalStore(
    useCallback((listener) => client.subscribe(listener), [client]),
    () => client.user,
  );
  const [ready, setReady] = useState(false);
  const [locale, setLocaleState] = useState(() => {
    const saved = client.store.read(LOCALE_KEY);
    return saved && schema.locales.includes(saved) ? saved : schema.default_locale;
  });
  const [dataVersion, setDataVersion] = useState(0);

  useEffect(() => {
    void client.restore().finally(() => setReady(true));
  }, [client]);

  const strings = customization.strings?.[locale] ?? stringsFor(locale);
  const format = useMemo(
    () => new ValueFormat(locale, schema.default_locale, strings, customization.enumLabels),
    [locale, schema.default_locale, strings, customization.enumLabels],
  );
  // Nouveau cache à chaque session, langue ou écriture.
  const titles = useMemo(
    () => new TitleCache(client, schema, format),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [client, schema, format, user, dataVersion],
  );
  const forge: Forge = useMemo(
    () => ({
      schema,
      client,
      customization,
      format,
      strings,
      titles,
      user,
      locale,
      setLocale: (l: string) => {
        client.store.write(LOCALE_KEY, l);
        setLocaleState(l);
      },
      dataVersion,
      notifyChange: () => setDataVersion((v) => v + 1),
    }),
    [schema, client, customization, format, strings, titles, user, locale, dataVersion],
  );
  const router = useMemo(
    () =>
      initialPath === undefined
        ? createBrowserRouter(routes)
        : createMemoryRouter(routes, { initialEntries: [initialPath] }),
    [initialPath],
  );

  return (
    <MantineProvider theme={buildTheme(customization.theme)} defaultColorScheme="auto">
      <DatesProvider settings={{ locale: locale.split('_')[0], firstDayOfWeek: 1 }}>
        <ForgeContext.Provider value={forge}>
          <ModalsProvider>
            <Notifications position="top-right" />
            {ready ? <RouterProvider router={router} /> : <Loading />}
          </ModalsProvider>
        </ForgeContext.Provider>
      </DatesProvider>
    </MantineProvider>
  );
}
