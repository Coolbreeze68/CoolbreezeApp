import {
  Alert,
  Anchor,
  Badge,
  Button,
  Center,
  Group,
  Loader,
  Pagination,
  Stack,
  Text,
  ThemeIcon,
  Title,
  useComputedColorScheme,
} from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { IconAlertTriangle, IconCheck, IconDatabaseOff, IconMinus } from '@tabler/icons-react';
import type { ReactNode } from 'react';
import { Link } from 'react-router';

import { ApiError, type Json } from '../api/client';
import { useForge } from '../context';
import { useRecordTitle } from '../hooks';
import { enumColor } from '../palette';
import { paths } from '../paths';
import type { ColumnSchema, TableSchema } from '../schema';
import { allows, findTable } from '../schema';
import { jsonToIds } from '../values';

/** Message d'une erreur, lisible par l'utilisateur. */
export function useErrorMessage() {
  const { strings } = useForge();
  return (error: unknown): string => {
    if (error instanceof ApiError) {
      if (error.status === 0) return strings.networkError;
      if (error.status === 403) return strings.forbidden;
      if (error.status === 404) return strings.notFound;
      if (error.message) return error.message;
    }
    return strings.unexpectedError;
  };
}

/** Notifications de succès et d'erreur. */
export function useNotify() {
  const message = useErrorMessage();
  return {
    success: (text: string) =>
      notifications.show({ message: text, color: 'teal', icon: <IconCheck size={18} /> }),
    error: (error: unknown) =>
      notifications.show({
        message: typeof error === 'string' ? error : message(error),
        color: 'red',
        icon: <IconAlertTriangle size={18} />,
      }),
  };
}

export function ErrorView({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  const { strings } = useForge();
  const message = useErrorMessage();
  return (
    <Alert color="red" icon={<IconAlertTriangle />} m="md" title={message(error)}>
      {onRetry && (
        <Button variant="light" color="red" size="xs" mt="xs" onClick={onRetry}>
          {strings.retry}
        </Button>
      )}
    </Alert>
  );
}

export const Loading = () => (
  <Center p="xl">
    <Loader type="dots" />
  </Center>
);

export function Empty({ children }: { children?: ReactNode }) {
  const { strings } = useForge();
  return (
    <Stack align="center" gap="xs" p="xl">
      <ThemeIcon size={48} radius="xl" variant="light" color="gray">
        <IconDatabaseOff size={26} />
      </ThemeIcon>
      <Text c="dimmed">{children ?? strings.noResults}</Text>
    </Stack>
  );
}

export const NotFound = () => {
  const { strings } = useForge();
  return <Empty>{strings.notFound}</Empty>;
};

/** En-tête de page : titre, sous-titre et actions. */
export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <Group justify="space-between" align="flex-end" mb="lg" wrap="wrap" gap="sm">
      <div>
        <Title order={2}>{title}</Title>
        {subtitle && (
          <Text c="dimmed" size="sm">
            {subtitle}
          </Text>
        )}
      </div>
      {actions && <Group gap="xs">{actions}</Group>}
    </Group>
  );
}

/** Navigation entre les pages d'une liste : « 26–50 sur 112 ». */
export function Pager({
  page,
  pages,
  first,
  last,
  total,
  onPage,
}: {
  page: number;
  pages: number;
  first: number;
  last: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const { strings } = useForge();
  return (
    <Group justify="space-between" p="sm">
      <Text size="sm" c="dimmed">
        {strings.range(first, last, total)}
      </Text>
      {pages > 1 && <Pagination size="sm" value={page} total={pages} onChange={onPage} />}
    </Group>
  );
}

/** Valeur d'énumération : pastille de couleur et libellé (texte neutre). */
export function EnumBadge({
  table,
  column,
  value,
}: {
  table: TableSchema;
  column: ColumnSchema;
  value: string;
}) {
  const { format } = useForge();
  const scheme = useComputedColorScheme('light');
  return (
    <Badge
      variant="dot"
      color={enumColor(column.values, value, scheme)}
      styles={{ label: { textTransform: 'none', fontWeight: 500 } }}
    >
      {format.enumLabel(table, column, value)}
    </Badge>
  );
}

/** Intitulé d'un enregistrement référencé ; lien vers sa fiche si `link`. */
export function RecordLink({ table, id, link = true }: { table: string; id: number; link?: boolean }) {
  const forge = useForge();
  const title = useRecordTitle(table, id);
  const schema = findTable(forge.schema, table);
  const readable = schema !== undefined && allows(schema, forge.user?.roles ?? [], 'read');
  if (!link || !readable) return <>{title}</>;
  return (
    <Anchor component={Link} to={paths.record(table, id)} onClick={(e) => e.stopPropagation()}>
      {title}
    </Anchor>
  );
}

/**
 * Valeur d'une colonne : texte mis en forme, énumérations en pastilles,
 * références cliquables, ou affichage personnalisé (`ForgeCustomization.cells`).
 */
export function ValueView({
  table,
  column,
  record,
  links = true,
}: {
  table: TableSchema;
  column: ColumnSchema;
  record: Json;
  links?: boolean;
}) {
  const { format, customization } = useForge();
  const value = record[column.name];
  const Custom = customization.cells?.[`${table.name}.${column.name}`];
  if (Custom) return <Custom table={table} column={column} record={record} value={value} />;
  if (value === null || value === undefined || value === '') {
    return <IconMinus size={14} color="var(--mantine-color-dimmed)" aria-label="—" />;
  }
  switch (column.type) {
    case 'enum':
      return <EnumBadge table={table} column={column} value={String(value)} />;
    case 'boolean':
      return value ? (
        <ThemeIcon size="sm" radius="xl" color="teal" variant="light" aria-label={format.strings.yes}>
          <IconCheck size={14} />
        </ThemeIcon>
      ) : (
        <Text size="sm" c="dimmed">
          {format.strings.no}
        </Text>
      );
    case 'reference':
      return typeof value === 'number' && column.target ? (
        <RecordLink table={column.target} id={value} link={links} />
      ) : null;
    case 'reference_list':
      return column.target ? (
        <Group gap={4}>
          {jsonToIds(value).map((id) => (
            <Badge key={id} variant="light" styles={{ label: { textTransform: 'none' } }}>
              <RecordLink table={column.target!} id={id} link={links} />
            </Badge>
          ))}
        </Group>
      ) : null;
    case 'text':
      return (
        <Text size="sm" style={{ whiteSpace: 'pre-wrap' }} lineClamp={links ? undefined : 1}>
          {String(value)}
        </Text>
      );
    default:
      return <>{format.format(table, column, value)}</>;
  }
}
